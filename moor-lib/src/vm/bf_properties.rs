use std::sync::Arc;

use async_trait::async_trait;

use moor_value::util::bitenum::BitEnum;
use moor_value::var::error::Error::{E_INVARG, E_TYPE};
use moor_value::var::variant::Variant;
use moor_value::var::{v_bool, v_list, v_none, v_objid, v_string, Var};

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::vm::builtin::BfRet::{Error, Ret};
use crate::vm::builtin::{BfCallState, BfRet, BuiltinFunction};
use crate::vm::VM;
use moor_value::model::props::{PropAttrs, PropFlag};

// property_info (obj <object>, str <prop-name>)              => list\
//  {<owner>, <perms> }
async fn bf_property_info<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(Error(E_TYPE));
    };
    let property_info = bf_args
        .world_state
        .get_property_info(bf_args.perms().clone(), *obj, prop_name.as_str())
        .await?;
    let owner = property_info.owner.unwrap();
    let flags = property_info.flags.unwrap();

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

    Ok(Ret(v_list(vec![v_objid(owner), v_string(perms)])))
}
bf_declare!(property_info, bf_property_info);

enum InfoParseResult {
    Fail(moor_value::var::error::Error),
    Success(PropAttrs),
}

fn info_to_prop_attrs(info: &[Var]) -> InfoParseResult {
    if info.len() < 2 || info.len() > 3 {
        return InfoParseResult::Fail(E_INVARG);
    }
    let Variant::Obj(owner) = info[0].variant() else {
        return InfoParseResult::Fail(E_TYPE);
    };
    let Variant::Str(perms) = info[1].variant() else {
        return InfoParseResult::Fail(E_TYPE);
    };
    let name = if info.len() == 3 {
        let Variant::Str(name) = info[2].variant() else {
            return InfoParseResult::Fail(E_TYPE);
        };
        Some(name.to_string())
    } else {
        None
    };

    let mut flags = BitEnum::new();
    for c in perms.as_str().chars() {
        match c {
            'r' => flags |= PropFlag::Read,
            'w' => flags |= PropFlag::Write,
            'c' => flags |= PropFlag::Chown,
            _ => return InfoParseResult::Fail(E_INVARG),
        }
    }

    InfoParseResult::Success(PropAttrs {
        name,
        value: None,
        location: None,
        owner: Some(*owner),
        flags: Some(flags),
    })
}

async fn bf_set_property_info<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 3 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::List(info) = bf_args.args[2].variant() else {
        return Ok(Error(E_TYPE));
    };

    let attrs = match info_to_prop_attrs(&info[..]) {
        InfoParseResult::Fail(e) => {
            return Ok(Error(e));
        }
        InfoParseResult::Success(a) => a,
    };

    bf_args
        .world_state
        .set_property_info(bf_args.perms().clone(), *obj, prop_name.as_str(), attrs)
        .await?;
    Ok(Ret(v_list(vec![])))
}
bf_declare!(set_property_info, bf_set_property_info);

async fn bf_is_clear_property<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(Error(E_TYPE));
    };
    let is_clear = bf_args
        .world_state
        .is_property_clear(bf_args.perms().clone(), *obj, prop_name.as_str())
        .await?;
    Ok(Ret(v_bool(is_clear)))
}
bf_declare!(is_clear_property, bf_is_clear_property);

async fn bf_clear_property<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(Error(E_TYPE));
    };
    bf_args
        .world_state
        .clear_property(bf_args.perms().clone(), *obj, prop_name.as_str())
        .await?;
    Ok(Ret(v_list(vec![])))
}
bf_declare!(set_clear_property, bf_clear_property);

// add_property (obj <object>, str <prop-name>, <value>, list <info>) => none
async fn bf_add_property<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 4 {
        return Ok(Error(E_INVARG));
    }

    let (Variant::Obj(location), Variant::Str(name), value, Variant::List(info)) = (bf_args.args[0].variant(),
        bf_args.args[1].variant(), bf_args.args[2].clone(), bf_args.args[3].variant()) else {
        return Ok(Error(E_INVARG));
    };

    let attrs = match info_to_prop_attrs(&info[..]) {
        InfoParseResult::Fail(e) => {
            return Ok(Error(e));
        }
        InfoParseResult::Success(a) => a,
    };

    bf_args
        .world_state
        .define_property(
            bf_args.perms().clone(),
            *location,
            *location,
            name.as_str(),
            bf_args.perms().task_perms().obj,
            attrs.flags.unwrap(),
            Some(value),
        )
        .await?;
    Ok(Ret(v_none()))
}
bf_declare!(add_property, bf_add_property);

async fn bf_delete_property<'a>(bf_args: &mut BfCallState<'a>) -> Result<BfRet, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(Error(E_INVARG));
    }
    let Variant::Obj(obj) = bf_args.args[0].variant() else {
        return Ok(Error(E_TYPE));
    };
    let Variant::Str(prop_name) = bf_args.args[1].variant() else {
        return Ok(Error(E_TYPE));
    };
    bf_args
        .world_state
        .delete_property(bf_args.perms().clone(), *obj, prop_name.as_str())
        .await?;
    Ok(Ret(v_list(vec![])))
}
bf_declare!(delete_property, bf_delete_property);
impl VM {
    pub(crate) fn register_bf_properties(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("property_info")] = Arc::new(Box::new(BfPropertyInfo {}));
        self.builtins[offset_for_builtin("set_property_info")] =
            Arc::new(Box::new(BfSetPropertyInfo {}));
        self.builtins[offset_for_builtin("is_clear_property")] =
            Arc::new(Box::new(BfIsClearProperty {}));
        self.builtins[offset_for_builtin("clear_property")] =
            Arc::new(Box::new(BfSetClearProperty {}));
        self.builtins[offset_for_builtin("add_property")] = Arc::new(Box::new(BfAddProperty {}));
        self.builtins[offset_for_builtin("delete_property")] =
            Arc::new(Box::new(BfDeleteProperty {}));

        Ok(())
    }
}
