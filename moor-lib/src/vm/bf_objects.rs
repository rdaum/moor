use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::var::Error::{E_INVARG, E_TYPE};
use crate::model::var::{v_err, Var, v_list, v_bool};
use crate::tasks::Sessions;
use crate::vm::activation::Activation;
use crate::vm::execute::{BfFunction, VM};

async fn bf_create(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    _args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
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
async fn bf_valid(
    ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let Var::Obj(obj) = &args[0] else {
        return Ok(v_err(E_TYPE));
    };
    let is_valid = ws.valid(*obj)?;
    Ok(v_bool(is_valid))
}
bf_declare!(valid, bf_valid);
/*
Function: obj parent (obj object)
Function: list children (obj object)
These functions return the parent and a list of the children of object, respectively. If object is not valid, then E_INVARG is raised.
 */

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
Changes what's location to be where. This is a complex process because a number of permissions checks and notifications must be performed. The actual movement takes place as described in the following paragraphs.
 */

async fn bf_verbs(
    ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let Var::Obj(obj) = &args[0] else {
        return Ok(v_err(E_TYPE));
    };
    let verbs = ws.verbs(*obj)?;
    let verbs = verbs
        .iter()
        .map(|v| Var::Str(v.names.first().unwrap().clone()))
        .collect();
    Ok(v_list(verbs))
}
bf_declare!(verbs, bf_verbs);

/*
Function: list properties (obj object)
Returns a list of the names of the properties defined directly on the given object, not inherited from its parent. If object is not valid, then E_INVARG is raised. If the programmer does not have read permission on object, then E_PERM is raised.
 */
async fn bf_properties(
    ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: Vec<Var>,
) -> Result<Var, anyhow::Error> {
    if args.len() != 1 {
        return Ok(v_err(E_INVARG));
    }
    let Var::Obj(obj) = &args[0] else {
        return Ok(v_err(E_TYPE));
    };
    let props = ws.properties(*obj)?;
    let props = props.iter().map(|p| Var::Str(p.0.clone())).collect();
    Ok(v_list(props))
}
bf_declare!(properties, bf_properties);

impl VM {
    pub(crate) fn register_bf_objects(&mut self) -> Result<(), anyhow::Error> {
        self.bf_funcs[offset_for_builtin("create")] = Arc::new(Box::new(BfCreate {}));
        self.bf_funcs[offset_for_builtin("valid")] = Arc::new(Box::new(BfValid {}));
        self.bf_funcs[offset_for_builtin("verbs")] = Arc::new(Box::new(BfVerbs {}));
        self.bf_funcs[offset_for_builtin("properties")] = Arc::new(Box::new(BfProperties {}));

        Ok(())
    }
}
