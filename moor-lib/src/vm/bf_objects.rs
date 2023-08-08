use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, error, trace};

use moor_value::var::error::Error::{E_INVARG, E_NACC, E_TYPE};
use moor_value::var::variant::Variant;
use moor_value::var::{v_bool, v_int, v_list, v_none, v_objid, v_str};

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::model::objects::ObjFlag;
use crate::model::ObjectError;
use crate::tasks::VerbCall;
use crate::vm::builtin::BfRet::{Error, Ret, VmInstr};
use crate::vm::builtin::{BfCallState, BfRet, BuiltinFunction};
use crate::vm::ExecutionResult::ContinueVerb;
use crate::vm::{ResolvedVerbCall, VM};

async fn bf_create<'a>(_bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    unimplemented!("create")
}
bf_declare!(create, bf_create);

/*
Function: none chparent (obj object, obj new-parent)
Changes the parent of object to be new-parent. If object is not valid, or if new-parent is neither valid nor equal to #-1, then E_INVARG is raised. If the programmer is neither a wizard or the owner of object, or if new-parent is not fertile (i.e., its `f' bit is not set) and the programmer is neither the owner of new-parent nor a wizard, then E_PERM is raised. If new-parent is equal to object or one of its current ancestors, E_RECMOVE is raised. If object or one of its descendants defines a property with the same name as one defined either on new-parent or on one of its ancestors, then E_INVARG is raised.
 */

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
    let parent = bf_args.world_state.parent_of(bf_args.perms(), *obj).await?;
    Ok(Ret(v_objid(parent)))
}
bf_declare!(parent, bf_parent);

async fn bf_children<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 1 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let children = bf_args
        .world_state
        .children_of(bf_args.perms(), *obj)
        .await?;
    debug!("Children: {:?} {:?}", obj, children);
    let children = children.iter().map(|c| v_objid(*c)).collect::<Vec<_>>();
    debug!("Children: {:?} {:?}", obj, children);
    Ok(Ret(v_list(children)))
}
bf_declare!(children, bf_children);

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

/*
Function: none move (obj what, obj where)
Changes what's location to be where. This is a complex process because a number of permissions
checks and notifications must be performed. The actual movement takes place as described in the
 following paragraphs.

<what> should be a valid object and <where> should be either a valid object or `#-1' (denoting a location of `nowhere'); otherwise `E_INVARG' is raised.  The
programmer must be either the owner of <what> or a wizard; otherwise, `E_PERM' is raised.

If <where> is a valid object, then the verb-call

    <where>:accept(<what>)

is performed before any movement takes place.  If the verb returns a false value and the programmer is not a wizard, then <where> is considered to have refused
entrance to <what>; `move()' raises `E_NACC'.  If <where> does not define an `accept' verb, then it is treated as if it defined one that always returned false.

If moving <what> into <where> would create a loop in the containment hierarchy (i.e., <what> would contain itself, even indirectly), then `E_RECMOVE' is raised
instead.

The `location' property of <what> is changed to be <where>, and the `contents' properties of the old and new locations are modified appropriately.  Let
<old-where> be the location of <what> before it was moved.  If <old-where> is a valid object, then the verb-call

    <old-where>:exitfunc(<what>)

is performed and its result is ignored; it is not an error if <old-where> does not define a verb named `exitfunc'.  Finally, if <where> and <what> are still
valid objects, and <where> is still the location of <what>, then the verb-call

    <where>:enterfunc(<what>)

is performed and its result is ignored; again, it is not an error if <where> does not define a verb named `enterfunc'.
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
    let mut shortcircuit = false;
    loop {
        match tramp {
            BF_MOVE_TRAMPOLINE_START_ACCEPT => {
                match bf_args
                    .world_state
                    .find_method_verb_on(bf_args.perms(), *whereto, "accept")
                    .await
                {
                    Ok(dispatch) => {
                        let continuation_verb = ResolvedVerbCall {
                            permissions: bf_args.perms().clone(),
                            resolved_verb: dispatch,
                            call: VerbCall {
                                verb_name: "accept".to_string(),
                                location: *whereto,
                                this: *whereto,
                                player: bf_args.vm.top().player,
                                args: vec![v_objid(*what)],
                                caller: bf_args.vm.top().this,
                            },
                            command: None,
                        };
                        return Ok(VmInstr(ContinueVerb {
                            verb_call: continuation_verb,
                            trampoline: Some(BF_MOVE_TRAMPOLINE_MOVE_CALL_EXITFUNC),
                        }));
                    }
                    Err(ObjectError::VerbNotFound(_, _)) => {
                        if !bf_args.perms().has_flag(ObjFlag::Wizard) {
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
                if !result.is_true() && !bf_args.perms().has_flag(ObjFlag::Wizard) {
                    return Ok(Error(E_NACC));
                }

                // Otherwise, ask the world state to move the object.
                trace!(what = ?what, where_to = ?*whereto, tramp, "move: moving object & calling enterfunc");

                let original_location = bf_args
                    .world_state
                    .location_of(bf_args.perms(), *what)
                    .await?;

                // Failure here is likely due to permissions, so we'll just propagate that error.
                bf_args
                    .world_state
                    .move_object(bf_args.perms(), *what, *whereto)
                    .await?;

                // Now, prepare to call :exitfunc on the original location.
                match bf_args
                    .world_state
                    .find_method_verb_on(bf_args.perms(), original_location, "exitfunc")
                    .await
                {
                    Ok(dispatch) => {
                        let continuation_verb = ResolvedVerbCall {
                            permissions: bf_args.perms().clone(),
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
                        };
                        return Ok(VmInstr(ContinueVerb {
                            verb_call: continuation_verb,
                            trampoline: Some(BF_MOVE_TRAMPOLINE_CALL_ENTERFUNC),
                        }));
                    }
                    Err(ObjectError::VerbNotFound(_, _)) => {
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
                trace!(what = ?what, where_to = ?*whereto, tramp, "move: calling enterfunc");
                // Exitfunc has been called, and returned. Result is irrelevant. Prepare to call
                // :enterfunc on the destination.
                match bf_args
                    .world_state
                    .find_method_verb_on(bf_args.perms(), *whereto, "enterfunc")
                    .await
                {
                    Ok(dispatch) => {
                        let continuation_verb = ResolvedVerbCall {
                            permissions: bf_args.perms().clone(),
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
                        };
                        return Ok(VmInstr(ContinueVerb {
                            verb_call: continuation_verb,
                            trampoline: Some(3),
                        }));
                    }
                    Err(ObjectError::VerbNotFound(_, _)) => {
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
    let verbs = bf_args.world_state.verbs(bf_args.perms(), *obj).await?;
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
        .properties(bf_args.perms(), *obj)
        .await?;
    let props = props.iter().map(|p| v_str(&p.0)).collect();
    Ok(Ret(v_list(props)))
}
bf_declare!(properties, bf_properties);

impl VM {
    pub(crate) fn register_bf_objects(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("create")] = Arc::new(Box::new(BfCreate {}));
        self.builtins[offset_for_builtin("valid")] = Arc::new(Box::new(BfValid {}));
        self.builtins[offset_for_builtin("verbs")] = Arc::new(Box::new(BfVerbs {}));
        self.builtins[offset_for_builtin("properties")] = Arc::new(Box::new(BfProperties {}));
        self.builtins[offset_for_builtin("parent")] = Arc::new(Box::new(BfParent {}));
        self.builtins[offset_for_builtin("children")] = Arc::new(Box::new(BfChildren {}));
        self.builtins[offset_for_builtin("move")] = Arc::new(Box::new(BfMove {}));

        Ok(())
    }
}
