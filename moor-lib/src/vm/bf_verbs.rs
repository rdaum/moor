use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::verbs::VerbFlag;
use crate::tasks::Sessions;
use crate::var::error::Error::{E_INVARG, E_TYPE};
use crate::var::{v_err, v_list, v_objid, v_string, Var, Variant};
use crate::vm::activation::Activation;
use crate::vm::vm::{BfFunction, VM};

// verb_info (obj <object>, str <verb-desc>) ->  {<owner>, <perms>, <names>}
async fn bf_verb_info(
    ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(obj) = args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(verb_desc) = args[1].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let verb_desc = verb_desc.as_str();
    let verb_info = ws.get_verb(*obj, verb_desc)?;
    let owner = verb_info.attrs.owner.unwrap();
    let perms = verb_info.attrs.flags.unwrap();
    let names = verb_info.names;

    let mut perms_string = String::new();
    if perms.contains(VerbFlag::Read) {
        perms_string.push('r');
    }
    if perms.contains(VerbFlag::Write) {
        perms_string.push('w');
    }
    if perms.contains(VerbFlag::Exec) {
        perms_string.push('x');
    }
    if perms.contains(VerbFlag::Debug) {
        perms_string.push('d');
    }
    let result = v_list(vec![
        v_objid(owner),
        v_string(perms_string),
        v_list(names.iter().map(|s| v_string(s.clone())).collect()),
    ]);
    Ok(result)
}
bf_declare!(verb_info, bf_verb_info);

impl VM {
    pub(crate) fn register_bf_verbs(&mut self) -> Result<(), anyhow::Error> {
        self.bf_funcs[offset_for_builtin("verb_info")] = Arc::new(Box::new(BfVerbInfo {}));

        Ok(())
    }
}
