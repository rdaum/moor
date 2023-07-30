use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use tracing::debug;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::model::r#match::{ArgSpec, PrepSpec, VerbArgsSpec};
use crate::model::verbs::VerbFlag;
use crate::tasks::Sessions;
use crate::util::bitenum::BitEnum;
use crate::var::error::Error::{E_INVARG, E_TYPE};
use crate::var::{v_err, v_list, v_objid, v_str, v_string, Var, Variant, VAR_NONE};
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

// set_verb_info (obj <object>, str <verb-desc>, list <info>) => none
async fn bf_set_verb_info(
    ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 3 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(obj) = args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(verb_name) = args[1].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::List(info) = args[2].variant() else {
        return Ok(v_err(E_TYPE));
    };
    if info.len() != 3 {
        return Ok(v_err(E_INVARG));
    }
    match (info[0].variant(), info[1].variant(), info[2].variant()) {
        (Variant::Obj(owner), Variant::Str(perms_str), Variant::List(names)) => {
            let mut perms = BitEnum::new();
            for c in perms_str.chars() {
                match c {
                    'r' => perms |= VerbFlag::Read,
                    'w' => perms |= VerbFlag::Write,
                    'x' => perms |= VerbFlag::Exec,
                    'd' => perms |= VerbFlag::Debug,
                    _ => return Ok(v_err(E_INVARG)),
                }
            }
            let mut name_strings = vec![];
            for name in names {
                if let Variant::Str(s) = name.variant() {
                    name_strings.push(s.clone());
                } else {
                    return Ok(v_err(E_TYPE));
                }
            }

            ws.update_verb_info(
                *obj,
                verb_name,
                Some(*owner),
                Some(name_strings),
                Some(perms),
                None,
            )?;
            Ok(VAR_NONE)
        }
        _ => return Ok(v_err(E_INVARG)),
    }
}
bf_declare!(set_verb_info, bf_set_verb_info);

async fn bf_verb_args(
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
    let args = verb_info.attrs.args_spec.unwrap();

    // Output is {dobj, prep, iobj} as strings
    let result = v_list(vec![
        v_str(args.dobj.to_string()),
        v_str(args.prep.to_string()),
        v_str(args.iobj.to_string()),
    ]);
    Ok(result)
}
bf_declare!(verb_args, bf_verb_args);

// set_verb_args (obj <object>, str <verb-desc>, list <args>) => none
async fn bf_set_verb_args(
    ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 3 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(obj) = args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(verb_name) = args[1].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::List(verbinfo) = args[2].variant() else {
        return Ok(v_err(E_TYPE));
    };
    if verbinfo.len() != 3 {
        return Ok(v_err(E_INVARG));
    }
    match (
        verbinfo[0].variant(),
        verbinfo[1].variant(),
        verbinfo[2].variant(),
    ) {
        (Variant::Str(dobj_str), Variant::Str(prep_str), Variant::Str(iobj_str)) => {
            let Some(dobj) = ArgSpec::from_string(dobj_str) else {
                return Ok(v_err(E_INVARG));
            };
            let Some(prep) = PrepSpec::from_string(prep_str) else {
                return Ok(v_err(E_INVARG));
            };
            let Some(iobj) = ArgSpec::from_string(iobj_str) else {
                return Ok(v_err(E_INVARG));
            };
            let args = VerbArgsSpec { dobj, prep, iobj };
            debug!("Updating verb args for {} to {:?}", verb_name, args);
            ws.update_verb_info(*obj, verb_name, None, None, None, Some(args))?;
            Ok(VAR_NONE)
        }
        _ => return Ok(v_err(E_INVARG)),
    }
}
bf_declare!(set_verb_args, bf_set_verb_args);

impl VM {
    pub(crate) fn register_bf_verbs(&mut self) -> Result<(), anyhow::Error> {
        self.bf_funcs[offset_for_builtin("verb_info")] = Arc::new(Box::new(BfVerbInfo {}));
        self.bf_funcs[offset_for_builtin("set_verb_info")] = Arc::new(Box::new(BfSetVerbInfo {}));
        self.bf_funcs[offset_for_builtin("verb_args")] = Arc::new(Box::new(BfVerbArgs {}));
        self.bf_funcs[offset_for_builtin("set_verb_args")] = Arc::new(Box::new(BfSetVerbArgs {}));

        Ok(())
    }
}
