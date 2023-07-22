use crate::compiler::labels::Label;
use crate::model::objects::ObjFlag;
use crate::model::verbs::VerbInfo;
use crate::tasks::scheduler::TaskId;
use crate::util::bitenum::BitEnum;
use crate::var::error::Error;
use crate::var::error::Error::E_VARNF;
use crate::var::{v_int, v_list, v_objid, v_str, Objid, Var, VarType, NOTHING, VAR_NONE};
use crate::vm::opcode::{Binary, Op};

// {this, verb-name, programmer, verb-loc, player, line-number}
#[derive(Clone)]
pub struct Caller {
    pub this: Objid,
    pub verb_name: String,
    pub programmer: Objid,
    pub verb_loc: Objid,
    pub player: Objid,
    pub line_number: usize,
}

pub(crate) struct Activation {
    pub(crate) task_id: TaskId,
    pub(crate) binary: Binary,
    pub(crate) environment: Vec<Var>,
    pub(crate) valstack: Vec<Var>,
    pub(crate) pc: usize,
    pub(crate) temp: Var,
    pub(crate) caller_perms: Objid,
    pub(crate) this: Objid,
    pub(crate) player: Objid,
    pub(crate) player_flags: BitEnum<ObjFlag>,
    pub(crate) verb_info: VerbInfo,
    pub(crate) callers: Vec<Caller>,
}

impl Activation {
    pub fn new_for_method(
        task_id: TaskId,
        binary: Binary,
        caller: Objid,
        this: Objid,
        player: Objid,
        player_flags: BitEnum<ObjFlag>,
        verb_info: VerbInfo,
        args: &[Var],
        callers: Vec<Caller>,
    ) -> Result<Self, anyhow::Error> {
        let environment = vec![VAR_NONE; binary.var_names.width()];

        // Take a copy of the verb name because we're going to move verb_info.
        let verb_name = verb_info.names.first().unwrap().clone();

        let mut a = Activation {
            task_id,
            binary,
            environment,
            valstack: vec![],
            pc: 0,
            temp: VAR_NONE,
            caller_perms: caller,
            this,
            player,
            player_flags,
            verb_info,
            callers,
        };

        a.set_var("this", v_objid(this)).unwrap();
        a.set_var("player", v_objid(player)).unwrap();
        a.set_var("caller", v_objid(caller)).unwrap();
        a.set_var("NUM", v_int(VarType::TYPE_INT as i64)).unwrap();
        a.set_var("OBJ", v_int(VarType::TYPE_OBJ as i64)).unwrap();
        a.set_var("STR", v_int(VarType::TYPE_STR as i64)).unwrap();
        a.set_var("ERR", v_int(VarType::TYPE_ERR as i64)).unwrap();
        a.set_var("LIST", v_int(VarType::TYPE_LIST as i64)).unwrap();
        a.set_var("INT", v_int(VarType::TYPE_INT as i64)).unwrap();
        a.set_var("FLOAT", v_int(VarType::TYPE_FLOAT as i64))
            .unwrap();

        a.set_var("verb", v_str(verb_name.as_str())).unwrap();
        a.set_var("argstr", v_str("")).unwrap();
        a.set_var("args", v_list(args.into())).unwrap();
        a.set_var("iobjstr", v_str("")).unwrap();
        a.set_var("iobj", v_objid(NOTHING)).unwrap();
        a.set_var("dobjstr", v_str("")).unwrap();
        a.set_var("dobj", v_objid(NOTHING)).unwrap();
        a.set_var("prepstr", v_str("")).unwrap();

        Ok(a)
    }

    pub fn verb_name(&self) -> &str {
        self.verb_info.names.first().unwrap()
    }

    pub fn verb_definer(&self) -> Objid {
        self.verb_info.attrs.definer.unwrap()
    }

    pub fn verb_owner(&self) -> Objid {
        self.verb_info.attrs.owner.unwrap()
    }

    pub fn set_var(&mut self, name: &str, value: Var) -> Result<(), Error> {
        let n = self.binary.var_names.find_name_offset(name);
        if let Some(n) = n {
            self.environment[n] = value;
            Ok(())
        } else {
            Err(E_VARNF)
        }
    }

    pub fn next_op(&mut self) -> Option<Op> {
        if !self.pc < self.binary.main_vector.len() {
            return None;
        }
        let op = self.binary.main_vector[self.pc].clone();
        self.pc += 1;
        Some(op)
    }

    pub fn lookahead(&self) -> Option<Op> {
        self.binary.main_vector.get(self.pc).cloned()
    }

    pub fn skip(&mut self) {
        self.pc += 1;
    }

    pub fn pop(&mut self) -> Option<Var> {
        self.valstack.pop()
    }

    pub fn push(&mut self, v: Var) {
        self.valstack.push(v)
    }

    pub fn peek_at(&self, i: usize) -> Option<Var> {
        if !i < self.valstack.len() {
            return None;
        }
        Some(self.valstack[self.valstack.len() - i].clone())
    }

    pub fn peek_top(&self) -> Option<Var> {
        self.valstack.last().cloned()
    }

    pub fn peek(&self, width: usize) -> Vec<Var> {
        let l = self.valstack.len();
        Vec::from(&self.valstack[l - width..])
    }

    pub fn jump(&mut self, label_id: Label) {
        let label = &self.binary.jump_labels[label_id.0 as usize];
        self.pc = label.position.0 as usize;
    }
}