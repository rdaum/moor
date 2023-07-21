use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::bf_declare;
use crate::compiler::builtins::offset_for_builtin;
use crate::db::state::WorldState;
use crate::tasks::Sessions;
use crate::var::error::Error::{E_INVARG, E_RANGE, E_TYPE};
use crate::var::{v_err, v_int, v_list, Var, Variant};
use crate::vm::activation::Activation;
use crate::vm::execute::{BfFunction, VM};

async fn bf_is_member(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let (value, list) = (&args[0], &args[1]);
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

async fn bf_listinsert(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() < 2 || args.len() > 3 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value) = (&args[0], &args[1]);
    let Variant::List(list) = list.variant() else {
        return Ok(v_err(E_TYPE));
    };
    let mut new_list = list.clone();
    if args.len() == 2 {
        new_list.push(value.clone());
    } else {
        let index = args[2].variant();
        let Variant::Int(index) = index else {
            return Ok(v_err(E_TYPE));
        };
        let index = index - 1;
        new_list.insert(index as usize, value.clone());
    }
    Ok(v_list(new_list))
}
bf_declare!(listinsert, bf_listinsert);

async fn bf_listappend(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() < 2 || args.len() > 3 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value) = (&args[0], &args[1]);
    let Variant::List(list) = list.variant() else {
        return Ok(v_err(E_TYPE));
    };
    let mut new_list = list.clone();
    if args.len() == 2 {
        new_list.push(value.clone());
    } else {
        let index = args[2].variant();
        let Variant::Int(index) = index else {
            return Ok(v_err(E_TYPE));
        };
        let index = index - 1;
        new_list.insert(index as usize + 1, value.clone());
    }
    Ok(v_list(new_list))
}
bf_declare!(listappend, bf_listappend);

async fn bf_listdelete(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let (list, index) = (args[0].variant(), args[1].variant());
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
    let mut new_list = list.clone();
    new_list.remove(index as usize);
    Ok(v_list(new_list))
}
bf_declare!(listdelete, bf_listdelete);

async fn bf_listset(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 3 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value, index) = (args[0].variant(), &args[1], args[2].variant());
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
    let mut new_list = list.clone();
    new_list[index as usize] = value.clone();
    Ok(v_list(new_list))
}
bf_declare!(listset, bf_listset);

async fn bf_setadd(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value) = (args[0].variant(), &args[1]);
    let Variant::List(list) = list else {
        return Ok(v_err(E_TYPE));
    };
    let mut new_list = list.clone();
    if !new_list.contains(value) {
        new_list.push(value.clone());
    }
    Ok(v_list(new_list))
}
bf_declare!(setadd, bf_setadd);

async fn bf_setremove(
    _ws: &mut dyn WorldState,
    _frame: &mut Activation,
    _sess: Arc<RwLock<dyn Sessions>>,
    args: &[Var],
) -> Result<Var, anyhow::Error> {
    if args.len() != 2 {
        return Ok(v_err(E_INVARG));
    }
    let (list, value) = (args[0].variant(), &args[1]);
    let Variant::List(list) = list else {
        return Ok(v_err(E_TYPE));
    };
    let mut new_list = list.clone();
    if let Some(index) = new_list.iter().position(|x| x == value) {
        new_list.remove(index);
    }
    Ok(v_list(new_list))
}
bf_declare!(setremove, bf_setremove);

impl VM {
    pub(crate) fn register_bf_list_sets(&mut self) -> Result<(), anyhow::Error> {
        self.bf_funcs[offset_for_builtin("is_member")] = Arc::new(Box::new(BfIsMember {}));
        self.bf_funcs[offset_for_builtin("listinsert")] = Arc::new(Box::new(BfListinsert {}));
        self.bf_funcs[offset_for_builtin("listappend")] = Arc::new(Box::new(BfListappend {}));
        self.bf_funcs[offset_for_builtin("listdelete")] = Arc::new(Box::new(BfListdelete {}));
        self.bf_funcs[offset_for_builtin("listset")] = Arc::new(Box::new(BfListset {}));
        self.bf_funcs[offset_for_builtin("setadd")] = Arc::new(Box::new(BfSetadd {}));
        self.bf_funcs[offset_for_builtin("setremove")] = Arc::new(Box::new(BfSetremove {}));

        Ok(())
    }
}
