use std::sync::Arc;

use async_trait::async_trait;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::vm::builtin::{BfCallState, BuiltinFunction};
use crate::vm::VM;
use moor_value::var::error::Error::{E_INVARG, E_RANGE, E_TYPE};
use moor_value::var::variant::Variant;
use moor_value::var::{v_err, v_int, Var};

async fn bf_is_member<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let (value, list) = (&bf_args.args[0], &bf_args.args[1]);
    let Variant::List(list) = list.variant() else {
        return Ok(v_err(E_TYPE));
    };
    if list.contains(value) {
        Ok(v_int(1))
    } else {
        Ok(v_int(0))
    }
}
bf_declare!(is_member, bf_is_member);

async fn bf_listinsert<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value) = (&bf_args.args[0], &bf_args.args[1]);
    let Variant::List(list) = list.variant() else {
        return Ok(v_err(E_TYPE));
    };
    let new_list = if bf_args.args.len() == 2 {
        list.push(value)
    } else {
        let index = bf_args.args[2].variant();
        let Variant::Int(index) = index else {
            return Ok(v_err(E_TYPE));
        };
        let index = index - 1;
        list.insert(index as usize, value)
    };
    Ok(new_list)
}
bf_declare!(listinsert, bf_listinsert);

async fn bf_listappend<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() < 2 || bf_args.args.len() > 3 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value) = (&bf_args.args[0], &bf_args.args[1]);
    let Variant::List(list) = list.variant() else {
        return Ok(v_err(E_TYPE));
    };
    let new_list = if bf_args.args.len() == 2 {
        list.push(value)
    } else {
        let index = bf_args.args[2].variant();
        let Variant::Int(index) = index else {
            return Ok(v_err(E_TYPE));
        };
        let index = index - 1;
        list.insert(index as usize, value)
    };
    Ok(new_list)
}
bf_declare!(listappend, bf_listappend);

async fn bf_listdelete<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let (list, index) = (bf_args.args[0].variant(), bf_args.args[1].variant());
    let Variant::List(list) = list else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Int(index) = index else {
        return Ok(v_err(E_TYPE));
    };
    if *index < 1 || *index > list.len() as i64 {
        return Ok(v_err(E_RANGE));
    }
    let index = index - 1;
    Ok(list.remove_at(index as usize))
}
bf_declare!(listdelete, bf_listdelete);

async fn bf_listset<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 3 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value, index) = (
        bf_args.args[0].variant(),
        &bf_args.args[1],
        bf_args.args[2].variant(),
    );
    let Variant::List(list) = list else {
        return Ok(v_err(E_TYPE));
    };
    let Variant::Int(index) = index else {
        return Ok(v_err(E_TYPE));
    };
    if *index < 1 || *index > list.len() as i64 {
        return Ok(v_err(E_RANGE));
    }
    let index = index - 1;
    Ok(list.set(index as usize, value))
}
bf_declare!(listset, bf_listset);

async fn bf_setadd<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value) = (bf_args.args[0].variant(), &bf_args.args[1]);
    let Variant::List(list) = list else {
        return Ok(v_err(E_TYPE));
    };
    if !list.contains(value) {
        return Ok(list.push(value));
    }
    Ok(bf_args.args[0].clone())
}
bf_declare!(setadd, bf_setadd);

async fn bf_setremove<'a>(bf_args: &mut BfCallState<'a>) -> Result<Var, anyhow::Error> {
    if bf_args.args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value) = (bf_args.args[0].variant(), &bf_args.args[1]);
    let Variant::List(list) = list else {
        return Ok(v_err(E_TYPE));
    };
    Ok(list.setremove(value))
}
bf_declare!(setremove, bf_setremove);

impl VM {
    pub(crate) fn register_bf_list_sets(&mut self) -> Result<(), anyhow::Error> {
        self.builtins[offset_for_builtin("is_member")] = Arc::new(Box::new(BfIsMember {}));
        self.builtins[offset_for_builtin("listinsert")] = Arc::new(Box::new(BfListinsert {}));
        self.builtins[offset_for_builtin("listappend")] = Arc::new(Box::new(BfListappend {}));
        self.builtins[offset_for_builtin("listdelete")] = Arc::new(Box::new(BfListdelete {}));
        self.builtins[offset_for_builtin("listset")] = Arc::new(Box::new(BfListset {}));
        self.builtins[offset_for_builtin("setadd")] = Arc::new(Box::new(BfSetadd {}));
        self.builtins[offset_for_builtin("setremove")] = Arc::new(Box::new(BfSetremove {}));

        Ok(())
    }
}
