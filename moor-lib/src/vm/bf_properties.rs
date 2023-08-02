use std::sync::Arc;

use async_trait::async_trait;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::model::props::{PropAttrs, PropFlag};
use crate::util::bitenum::BitEnum;
use crate::values::error::Error::{E_INVARG, E_TYPE};
use crate::values::var::{v_err, v_int, v_list, v_objid, v_string, Var};
use crate::values::variant::Variant;
use crate::vm::builtin::{BfCallState, BuiltinFunction};
use crate::vm::VM;

// property_info (obj <object>, str <prop-name>)              => list\
//  {<owner>, <perms> }
async fn bf_property_info<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let property_info =
        bf_args
            .world_state
            .get_property_info(bf_args.perms(), *obj, prop_name.as_str())?;
    let owner = property_info.owner.unwrap();
    let flags = property_info.flags.unwrap();
    let name = property_info.name.unwrap();

    // Turn perm flags into string: r w c
    let mut perms = String::new();
    if flags.contains(PropFlag::Read) {
        perms.push('r');
    }
    if flags.contains(PropFlag::Write) {
        perms.push('w');
    }
    if flags.contains(PropFlag::Chown) {
        perms.push('c');
    }

    Ok(v_list(vec![
        v_objid(owner),
        v_string(perms),
        v_string(name),
    ]))
}
bf_declare!(property_info, bf_property_info);

async fn bf_set_property_info<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 3 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::List(info) = bf_args.args[2].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Obj(owner) = info[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(perms) = info[1].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(name) = info[2].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let mut flags = BitEnum::new();
    for c in perms.as_str().chars() {
        match c {
            'r' => flags |= PropFlag::Read,
            'w' => flags |= PropFlag::Write,
            'c' => flags |= PropFlag::Chown,
            _ => return Ok(v_err(E_INVARG)),
        }
    }
    bf_args.world_state.set_property_info(
        bf_args.perms(),
        *obj,
        prop_name.as_str(),
        PropAttrs {
            name: Some(name.to_string()),
            value: None,
            location: None,
            owner: Some(*owner),
            flags: Some(flags),
            is_clear: None,
        },
    )?;
    Ok(v_list(vec![]))
}
bf_declare!(set_property_info, bf_set_property_info);

async fn bf_is_clear_property<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let property_info =
        bf_args
            .world_state
            .get_property_info(bf_args.perms(), *obj, prop_name.as_str())?;
    let is_clear = if property_info.is_clear.unwrap() {
        1
    } else {
        0
    };
    Ok(v_int(is_clear))
}
bf_declare!(is_clear_property, bf_is_clear_property);

async fn bf_clear_property<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(v_err(E_TYPE));
    };
    bf_args.world_state.set_property_info(
        bf_args.perms(),
        *obj,
        prop_name.as_str(),
        PropAttrs {
            name: None,
            value: None,
            location: None,
            owner: None,
            flags: None,
            is_clear: Some(true),
        },
    )?;
    Ok(v_list(vec![]))
}
bf_declare!(set_clear_property, bf_clear_property);

impl VM {
    pub(crate) fn register_bf_properties(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("property_info")] = Arc::new(Box::new(BfPropertyInfo {}));
        self.builtins[offset_for_builtin("set_property_info")] =
            Arc::new(Box::new(BfSetPropertyInfo {}));
        self.builtins[offset_for_builtin("is_clear_property")] =
            Arc::new(Box::new(BfIsClearProperty {}));
        self.builtins[offset_for_builtin("clear_property")] =
            Arc::new(Box::new(BfSetClearProperty {}));

        Ok(())
    }
}
