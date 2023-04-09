use crate::compiler::labels::Label;
use crate::model::objects::ObjFlag;
use crate::model::var::Error;
use crate::model::var::Error::E_VARNF;
use crate::model::var::{Objid, Var};
use crate::server::parse_cmd::ParsedCommand;
use crate::util::bitenum::BitEnum;
use crate::vm::opcode::{Binary, Op};

pub(crate) struct Activation {
    pub(crate) binary: Binary,
    pub(crate) environment: Vec<Var>,
    pub(crate) valstack: Vec<Var>,
    pub(crate) pc: usize,
    pub(crate) error_pc: usize,
    pub(crate) temp: Var,
    pub(crate) this: Objid,
    pub(crate) player: Objid,
    pub(crate) player_flags: BitEnum<ObjFlag>,
    pub(crate) verb_owner: Objid,
    pub(crate) definer: Objid,
    pub(crate) verb: String,
}

impl Activation {
    pub fn new_for_method(
        binary: Binary,
        caller: Objid,
        this: Objid,
        player: Objid,
        player_flags: BitEnum<ObjFlag>,
        verb_owner: Objid,
        definer: Objid,
        verb: String,
        args: Vec<Var>,
    ) -> Result<Self, anyhow::Error> {
        let environment = vec![Var::None; binary.var_names.width()];

        let mut a = Activation {
            binary,
            environment,
            valstack: vec![],
            pc: 0,
            error_pc: 0,
            temp: Var::None,
            this,
            player,
            player_flags,
            verb_owner,
            definer,
            verb: verb.clone(),
        };

        a.set_var("this", Var::Obj(this)).unwrap();
        a.set_var("player", Var::Obj(player)).unwrap();
        a.set_var("caller", Var::Obj(caller)).unwrap();
        a.set_var("NUM", Var::Int(0)).unwrap();
        a.set_var("OBJ", Var::Int(1)).unwrap();
        a.set_var("STR", Var::Int(2)).unwrap();
        a.set_var("ERR", Var::Int(3)).unwrap();
        a.set_var("LIST", Var::Int(4)).unwrap();
        a.set_var("INT", Var::Int(0)).unwrap();
        a.set_var("FLOAT", Var::Int(9)).unwrap();

        a.set_var("verb", Var::Str(verb)).unwrap();
        a.set_var("argstr", Var::Str(String::from(""))).unwrap();
        a.set_var("args", Var::List(args)).unwrap();
        a.set_var("iobjstr", Var::Str(String::from(""))).unwrap();
        a.set_var("iobj", Var::Obj(Objid(-1))).unwrap();
        a.set_var("dobjstr", Var::Str(String::from(""))).unwrap();
        a.set_var("dobj", Var::Obj(Objid(-1))).unwrap();
        a.set_var("prepstr", Var::Str(String::from(""))).unwrap();

        Ok(a)
    }

    pub fn new_for_command(
        binary: Binary,
        caller: Objid,
        this: Objid,
        player: Objid,
        player_flags: BitEnum<ObjFlag>,
        verb_owner: Objid,
        definer: Objid,
        parsed_cmd: &ParsedCommand,
    ) -> Result<Self, anyhow::Error> {
        let environment = vec![Var::None; binary.var_names.width()];

        let mut a = Activation {
            binary,
            environment,
            valstack: vec![],
            pc: 0,
            error_pc: 0,
            temp: Var::None,
            this,
            player,
            player_flags,
            verb_owner,
            definer,
            verb: parsed_cmd.verb.clone(),
        };

        a.set_var("this", Var::Obj(this)).unwrap();
        a.set_var("player", Var::Obj(player)).unwrap();
        a.set_var("caller", Var::Obj(caller)).unwrap();
        a.set_var("NUM", Var::Int(0)).unwrap();
        a.set_var("OBJ", Var::Int(1)).unwrap();
        a.set_var("STR", Var::Int(2)).unwrap();
        a.set_var("ERR", Var::Int(3)).unwrap();
        a.set_var("LIST", Var::Int(4)).unwrap();
        a.set_var("INT", Var::Int(0)).unwrap();
        a.set_var("FLOAT", Var::Int(9)).unwrap();

        a.set_var("verb", Var::Str(parsed_cmd.verb.clone()))
            .unwrap();
        a.set_var("argstr", Var::Str(parsed_cmd.argstr.clone()))
            .unwrap();
        a.set_var("args", Var::List(parsed_cmd.args.clone()))
            .unwrap();
        a.set_var("iobjstr", Var::Str(parsed_cmd.iobjstr.clone()))
            .unwrap();
        a.set_var("iobj", Var::Obj(parsed_cmd.iobj)).unwrap();
        a.set_var("dobjstr", Var::Str(parsed_cmd.dobjstr.clone()))
            .unwrap();
        a.set_var("dobj", Var::Obj(parsed_cmd.dobj)).unwrap();
        a.set_var("prepstr", Var::Str(parsed_cmd.prepstr.clone()))
            .unwrap();

        Ok(a)
    }

    fn set_var(&mut self, name: &str, value: Var) -> Result<(), Error> {
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

    pub fn poke(&mut self, p: usize, v: &Var) {
        let l = self.valstack.len() - 1;
        self.valstack[l - p] = v.clone();
    }

    pub fn stack_size(&self) -> usize {
        self.valstack.len()
    }

    pub fn jump(&mut self, label_id: Label) {
        let label = &self.binary.jump_labels[label_id.0 as usize];
        self.pc = label.position.0 as usize;
    }

    pub fn seek_finally(&self) -> Option<Label> {
        for i in (0..self.valstack.len()).rev() {
            if let Var::_Finally(label) = self.valstack[i] {
                return Some(label);
            }
        }
        None
    }
}
