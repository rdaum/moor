use crate::model::objects::ObjFlag;
use crate::model::permissions::Permissions;
use crate::model::props::{PropAttr, PropAttrs, PropFlag};
use crate::model::var::Error::{
    E_ARGS, E_INVARG, E_INVIND, E_PERM, E_PROPNF, E_RANGE, E_TYPE, E_VARNF, E_VERBNF,
};
use crate::model::var::Var::Obj;
use crate::model::var::{Error, Objid, Var};
use crate::model::verbs::{Program, VerbAttr};
use crate::model::ObjDB;
use crate::vm::opcode::{Binary, Op};
use anyhow::anyhow;
use bincode::config;
use bincode::config::Configuration;
use bincode::error::DecodeError;
use enumset::EnumSet;
use itertools::Itertools;

/* Reasons for executing a FINALLY handler; constants are stored in DB, don't change order */
const FINALLY_FALLTHROUGH: i64 = 0x00;
const FINALLY_RAISE: i64 = 0x01;
const FINALLY_UNCAUGHT: i64 = 0x02;
const FINALLY_RETURN: i64 = 0x03;
const FINALLY_ABORT: i64 = 0x04; /* This doesn't actually get you into a FINALLY... */
const FINALLY_EXIT: i64 = 0x65;

struct Activation {
    binary: Binary,
    environment: Vec<Var>,
    valstack: Vec<Var>,
    pc: usize,
    error_pc: usize,
    temp: Var,
    this: Objid,
    player: Objid,
    verb_owner: Objid,
    definer: Objid,

    verb: String,
    verb_names: Vec<String>,
}

impl Activation {
    pub fn new(
        program: &Program,
        this: Objid,
        player: Objid,
        verb_owner: Objid,
        definer: Objid,
        verb: String,
        verb_names: Vec<String>,
    ) -> Result<Self, anyhow::Error> {
        // TODO: move deserialization out into whatever does the actual verb retrieval?
        let slc = &program.0[..];
        let result: Result<(Binary, usize), DecodeError> =
            bincode::serde::decode_from_slice(slc, config::standard());
        let Ok((binary, size)) = result else {
            return Err(anyhow!("Invalid opcodes in binary program stream"));
        };

        let environment = vec![Var::None; binary.var_names.len()];
        Ok(Activation {
            binary,
            environment,
            valstack: vec![],
            pc: 0,
            error_pc: 0,
            temp: Var::None,
            this,
            player,
            verb_owner,
            definer,
            verb,
            verb_names,
        })
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
        if !self.pc + 1 < self.binary.main_vector.len() {
            return None;
        }
        Some(self.binary.main_vector[self.pc + 1].clone())
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

    pub fn peek(&self, width: usize) -> Vec<Var> {
        let l = self.valstack.len();
        Vec::from(&self.valstack[l - width..])
    }

    pub fn poke(&mut self, p: usize, v: &Var) {
        let l = self.valstack.len();
        self.valstack[l - p] = v.clone()
    }

    pub fn jump(&mut self, label_id: usize) {
        let label = &self.binary.jump_labels[label_id];
        self.pc += label.offset;
    }

    pub fn rewind(&mut self, amt: usize) {
        self.pc -= amt;
    }
}

struct VM {
    // Activation stack.
    stack: Vec<Activation>,
}

macro_rules! binary_bool_op {
    ( $act:ident, $op:tt ) => {
        let rhs = $act.pop();
        let lhs = $act.pop();
        let result = if lhs $op rhs { 1 } else { 0 };
        $act.push(&Var::Int(result))
    };
}

macro_rules! binary_var_op {
    ( $act:ident, $op:tt ) => {
        let rhs = $act.pop();
        let lhs = $act.pop();
        let result = lhs.$op(&rhs);
        $act.push(&result)
    };
}

impl VM {
    pub fn raise_error(&mut self, err: Error) {}

    fn top_mut(&mut self) -> &mut Activation {
        self.stack.last_mut().expect("activation stack underflow")
    }

    fn top(&self) -> &Activation {
        self.stack.last().expect("activation stack underflow")
    }

    fn pop(&mut self) -> Var {
        self.top_mut().pop().expect("value stack underflow")
    }

    fn push(&mut self, v: &Var) {
        self.top_mut().push(v.clone())
    }

    fn next_op(&mut self) -> Option<Op> {
        self.top_mut().next_op()
    }

    fn jump(&mut self, label: usize) {
        self.top_mut().jump(label)
    }

    fn get_env(&mut self, id: usize) -> Var {
        self.top().environment[id].clone()
    }

    fn set_env(&mut self, id: usize, v: &Var) {
        self.top_mut().environment[id] = v.clone();
    }

    fn rewind(&mut self, amt: usize) {
        self.top_mut().rewind(amt);
    }

    fn peek(&self, amt: usize) -> Vec<Var> {
        self.top().peek(amt)
    }

    fn peek_top(&self) -> Var {
        self.top().peek(0)[0].clone()
    }

    fn poke(&mut self, pos: usize, v: &Var) {
        self.top_mut().poke(pos, v);
    }

    fn get_prop(
        &mut self,
        db: &dyn ObjDB,
        player_flags: EnumSet<ObjFlag>,
        propname: Var,
        obj: Var,
    ) -> Var {
        let Var::Str(propname) = propname else {
            return Var::Err(E_TYPE);
        };

        let Var::Obj(obj) = obj else {
            return Var::Err(E_INVIND);
        };

        // TODO builtin properties!

        let find = db
            .find_property(
                obj,
                propname.as_str(),
                PropAttr::Owner | PropAttr::Flags | PropAttr::Location | PropAttr::Value,
            )
            .expect("db fail");
        let prop_val = match find {
            None => Var::Err(E_PROPNF),
            Some(p) => {
                if !db.property_allows(
                    PropFlag::Read.into(),
                    obj,
                    player_flags,
                    p.attrs.flags.unwrap(),
                    p.attrs.owner.unwrap(),
                ) {
                    Var::Err(E_PERM)
                } else {
                    match p.attrs.value {
                        None => Var::Err(E_PROPNF),
                        Some(p) => p,
                    }
                }
            }
        };
        return prop_val;
    }

    pub fn call_verb(
        &mut self,
        db: &mut impl ObjDB,
        vname: &str,
        args: Var,
        do_pass: bool,
    ) -> Result<Var, anyhow::Error> {
        // TODO do_pass get parent and delegate there instead.
        // Requires adding db.object_parent.
        let h = db.find_callable_verb(
            self.top().this,
            vname,
            VerbAttr::Program | VerbAttr::Flags | VerbAttr::Owner | VerbAttr::Definer,
        )?;
        let Some(h) = h else {
            return Ok(Var::Err(E_VERBNF));
        };

        let a = Activation::new(
            &h.attrs.program.unwrap(),
            self.top().this,
            self.top().player,
            h.attrs.owner.unwrap(),
            h.attrs.definer.unwrap(),
            String::from(vname),
            h.names,
        )?;

        // TODO copy this, caller,player argstr,dobj,dobjstr, etc. into correct slots.
        // and set verb & args slots
        self.stack.push(a);

        Ok(Var::Err(Error::E_NONE))
    }

    pub fn exec(
        &mut self,
        db: &mut impl ObjDB,
        player: Objid,
        player_flags: EnumSet<ObjFlag>,
    ) -> Result<(), anyhow::Error> {
        let op = self.next_op();
        let Some(op) = op else {
            // Execution complete.
            // TODO is this an error?
            return Ok(())
        };
        match op {
            Op::If(label) | Op::Eif(label) | Op::IfQues(label) | Op::While(label) => {
                let cond = self.pop();
                if cond.is_true() {
                    self.jump(label);
                }
            }
            Op::Jump { label } => {
                self.jump(label);
            }
            Op::WhileId { id, label } => {
                self.set_env(id, &self.peek_top());
                let cond = self.pop();
                if cond.is_true() {
                    self.jump(label);
                }
            }
            Op::ForList { label, id } => {
                let peek = self.peek(2);
                let (count, list) = (&peek[1], &peek[0]);
                let Var::Int(count) = count else {
                    self.raise_error(Error::E_TYPE);
                    self.pop();
                    self.pop();
                    self.jump(label);
                    return Ok(())
                };
                let Var::List(l) = list else {
                    self.raise_error(Error::E_TYPE);
                    self.pop();
                    self.pop();
                    self.jump(label);
                    return Ok(())
                };

                if *count as usize > l.len() {
                    self.pop();
                    self.pop();
                    self.jump(label);
                } else {
                    self.set_env(id, &l[*count as usize]);
                    self.poke(0, &Var::Int(*count + 1));
                    self.rewind(3);
                }
            }
            Op::ForRange { label, id } => {
                let peek = self.peek(2);
                let (to, from) = (&peek[1], &peek[0]);

                // TODO: LambdaMOO has special handling for MAXINT/MAXOBJ
                // Given we're 64-bit this is exceedling unlikely to ever be a concern for us, but
                // we also don't want to *crash* on obscene values, so impl that here.

                let next_val = match (to, from) {
                    (Var::Int(to_i), Var::Int(from_i)) => {
                        if to_i > from_i {
                            self.pop();
                            self.pop();
                            self.jump(label);
                            return Ok(());
                        }
                        Var::Int(from_i + 1)
                    }
                    (Var::Obj(to_o), Var::Obj(from_o)) => {
                        if to_o.0 > from_o.0 {
                            self.pop();
                            self.pop();
                            self.jump(label);
                            return Ok(());
                        }
                        Var::Obj(Objid(from_o.0 + 1))
                    }
                    (_, _) => {
                        self.raise_error(E_TYPE);
                        return Ok(());
                    }
                };

                self.set_env(id, &from);
                self.poke(1, &next_val);
                self.rewind(3);
            }
            Op::Pop => {
                self.pop();
            }
            Op::Imm(slot) => {
                // Peek ahead to see if the next operation is 'pop' and if so, just throw away.
                // MOO uses this to optimize verbdoc/comments, etc.
                match self.top().lookahead() {
                    Some(Op::Pop) => {
                        // skip
                        self.top_mut().skip();
                        return Ok(());
                    }
                    _ => {}
                }
                let value = self.top().binary.literals[slot].clone();
                self.push(&value);
            }
            Op::MkEmptyList => self.push(&Var::List(vec![])),
            Op::ListAddTail => {
                let tail = self.pop();
                let list = self.pop();
                let Var::List(list) = list else {
                    self.push(&Var::Err(E_TYPE));
                    return Ok(());
                };

                // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA

                let mut new_list = list.clone();
                new_list.push(tail);
                self.push(&Var::List(new_list))
            }
            Op::ListAppend => {
                let tail = self.pop();
                let list = self.pop();
                let Var::List(list) = list else {
                    self.push(&Var::Err(E_TYPE));
                    return Ok(());
                };

                let Var::List(tail) = tail else {
                    self.push(&Var::Err(E_TYPE));
                    return Ok(());
                };

                // TODO: quota check SVO_MAX_LIST_CONCAT -> E_QUOTA
                let new_list = list.into_iter().chain(tail.into_iter());
                self.push(&Var::List(new_list.collect()))
            }
            Op::IndexSet => {
                // collection[index] = value
                let value = self.pop(); /* rhs value */
                let index = self.pop(); /* index, must be int */
                let list = self.pop(); /* lhs except last index, should be list or str */

                let nval = match (list, index) {
                    (Var::List(l), Var::Int(i)) => {
                        if i < 0 || !i < l.len() as i64 {
                            self.push(&Var::Err(E_RANGE));
                            return Ok(());
                        }

                        let mut nval = l.clone();
                        nval[i as usize] = value;
                        Var::List(nval)
                    }
                    (Var::Str(s), Var::Int(i)) => {
                        if i < 0 || !i < s.len() as i64 {
                            self.push(&Var::Err(E_RANGE));
                            return Ok(());
                        }

                        let Var::Str(value) = value else {
                            self.push(&Var::Err(E_INVARG));
                            return Ok(())
                        };

                        if value.len() != 1 {
                            self.push(&Var::Err(E_INVARG));
                            return Ok(());
                        }

                        let i = i as usize;
                        let (mut head, tail) = (String::from(&s[0..i]), &s[i + 1..]);
                        head.push_str(&value[0..1]);
                        head.push_str(tail);
                        Var::Str(head)
                    }
                    (_, _) => {
                        self.push(&Var::Err(E_TYPE));
                        return Ok(());
                    }
                };
                self.push(&nval);
            }
            Op::MakeSingletonList => {
                let v = self.pop();
                self.push(&Var::List(vec![v]))
            }
            Op::CheckListForSplice => {}
            Op::PutTemp => {
                self.top_mut().temp = self.peek_top();
            }
            Op::PushTemp => {
                let tmp = self.top().temp.clone();
                self.push(&tmp);
                self.top_mut().temp = Var::None;
            }
            Op::Eq => {
                binary_bool_op!(self, ==);
            }
            Op::Ne => {
                binary_bool_op!(self, !=);
            }
            Op::Gt => {
                binary_bool_op!(self, >);
            }
            Op::Lt => {
                binary_bool_op!(self, <);
            }
            Op::Ge => {
                binary_bool_op!(self, >=);
            }
            Op::Le => {
                binary_bool_op!(self, <=);
            }
            Op::In => {
                let lhs = self.pop();
                let rhs = self.pop();
                self.push(&lhs.has_member(&rhs));
            }
            Op::Mul => {
                binary_var_op!(self, mul);
            }
            Op::Sub => {
                binary_var_op!(self, sub);
            }
            Op::Div => {
                binary_var_op!(self, div);
            }
            Op::Add => {
                binary_var_op!(self, add);
            }
            Op::Exp => {
                binary_var_op!(self, pow);
            }
            Op::Mod => {
                binary_var_op!(self, modulus);
            }
            Op::And(label) => {
                let v = self.pop().is_true();
                if !v {
                    self.jump(label)
                }
            }
            Op::Or(label) => {
                let v = self.pop().is_true();
                if v {
                    self.jump(label)
                }
            }
            Op::Not => {
                let v = !self.pop().is_true();
                self.push(&Var::Int(if v { 1 } else { 0 }));
            }
            Op::UnaryMinus => {
                let v = self.pop();
                self.push(&v.negative())
            }
            Op::Ref => {
                let index = self.pop();
                let l = self.pop();
                let Var::Int(index) = index else {
                    self.push(&Var::Err(E_TYPE));
                    return Ok(())
                };
                self.push(&l.index(index as usize));
            }
            Op::Push(ident) => {
                let v = self.get_env(ident);
                match v {
                    Var::None => self.push(&Var::Err(E_VARNF)),
                    _ => self.push(&v),
                }
            }
            Op::Put(ident) => {
                let v = self.pop();
                self.set_env(ident, &v);
            }
            Op::PushRef => {
                let peek = self.peek(2);
                let (index, list) = (peek[1].clone(), peek[0].clone());
                let v = match (index, list) {
                    (Var::Int(index), Var::List(list)) => {
                        if index <= 0 || !index < list.len() as i64 {
                            Var::Err(E_RANGE)
                        } else {
                            list[index as usize].clone()
                        }
                    }
                    (_, _) => Var::Err(E_TYPE),
                };
                self.push(&v);
            }
            Op::RangeRef => {
                let (to, from, base) = (self.pop(), self.pop(), self.pop());
                let result = match (to, from, base) {
                    (Var::Int(to), Var::Int(from), Var::Str(base)) => {
                        if to < 0
                            || !to < base.len() as i64
                            || from < 0
                            || !from < base.len() as i64
                        {
                            Var::Err(E_RANGE)
                        } else {
                            let (from, to) = (from as usize, to as usize);
                            let substr = &base[from..to];
                            Var::Str(String::from(substr))
                        }
                    }
                    (Var::Int(to), Var::Int(from), Var::List(base)) => {
                        if to < 0
                            || !to < base.len() as i64
                            || from < 0
                            || !from < base.len() as i64
                        {
                            Var::Err(E_RANGE)
                        } else {
                            let (from, to) = (from as usize, to as usize);
                            let sublist = &base[from..to];
                            Var::List(Vec::from(sublist))
                        }
                    }
                    (_, _, _) => Var::Err(E_TYPE),
                };
                self.push(&result);
            }
            Op::GPut { id } => {
                self.set_env(id, &self.peek_top());
            }
            Op::GPush { id } => {
                let v = self.get_env(id);
                match v {
                    Var::None => self.push(&Var::Err(E_VARNF)),
                    _ => {
                        self.push(&v);
                    }
                }
            }
            Op::Length { id } => {
                let v = self.get_env(id);
                match v {
                    Var::Str(s) => self.push(&Var::Int(s.len() as i64)),
                    Var::List(l) => self.push(&Var::Int(l.len() as i64)),
                    _ => {
                        self.push(&Var::Err(E_TYPE));
                    }
                }
            }
            // TODO This is all very frobby and copied 1:1 from the MOO C source and is pretty much
            // guaranteed to not work the first ... N... times
            Op::Scatter {
                nargs,
                nreg: nreq,
                rest,
                id,
                label,
                done,
            } => {
                let have_rest = rest > nargs;
                let list = self.peek_top();
                let Var::List(list) = list else {
                    self.push(&Var::Err(E_TYPE));
                    return Ok(());
                };

                let len = list.len();
                if len < nreq || (!have_rest && len > nargs) {
                    self.push(&Var::Err(E_ARGS));
                    return Ok(());
                }

                let mut nopt_avail = len - nreq;
                let nrest = if have_rest && len > nargs {
                    len - nargs + 1
                } else {
                    0
                };
                let mut offset = 0;
                let mut whr = 0;
                for i in 1..nargs {
                    if i == rest {
                        // rest
                        let sublist = &list[i..i + nrest - 1];
                        let sublist = Var::List(Vec::from(sublist));
                        self.set_env(id, &sublist);
                    } else if label == 0 {
                        // required
                        self.set_env(id, &list[i + offset].clone());
                    } else {
                        // optional
                        if nopt_avail > 0 {
                            nopt_avail -= 1;

                            self.set_env(id, &list[i + offset]);
                        } else {
                            offset -= 1;
                            if whr == 0 && label != 1 {
                                whr = label;
                            }
                        }
                    }
                }
                if whr == 0 {
                    self.jump(done);
                } else {
                    self.jump(whr);
                }
            }

            Op::GetProp => {
                let (propname, obj) = (self.pop(), self.pop());
                let prop = self.get_prop(db, player_flags, propname, obj);
                self.push(&prop);
            }
            Op::PushGetProp => {
                let peeked = self.peek(2);
                let (propname, obj) = (peeked[0].clone(), peeked[1].clone());
                let pop = self.get_prop(db, player_flags, propname, obj);
                self.push(&pop);
            }
            Op::PutProp => {
                let (rhs, propname, obj) = (self.pop(), self.pop(), self.pop());
                let (propname, obj) = match (propname, obj) {
                    (Var::Str(propname), Var::Obj(obj)) => (propname, obj),
                    (_, _) => {
                        self.push(&Var::Err(E_TYPE));
                        return Ok(());
                    }
                };
                let h = db
                    .find_property(obj, propname.as_str(), PropAttr::Owner | PropAttr::Flags)
                    .expect("Unable to perform property lookup");

                // TODO handle built-in properties
                let Some(p) = h else {
                    self.push(&Var::Err(E_PROPNF));
                    return Ok(())
                };

                if !db.property_allows(
                    PropFlag::Write.into(),
                    obj,
                    player_flags,
                    p.attrs.flags.unwrap(),
                    p.attrs.owner.unwrap(),
                ) {
                    db.set_property(
                        p.pid,
                        obj,
                        rhs,
                        p.attrs.owner.unwrap(),
                        p.attrs.flags.unwrap(),
                    )
                    .expect("could not set property");
                    self.push(&Var::None);
                    return Ok(());
                }
            }
            Op::This => {
                self.push(&Var::Obj(self.top().this) );
            }

            Op::Fork { id, f_index } => {}
            Op::CallVerb => {
                let (args, verb, obj) = (self.pop(), self.pop(), self.pop());
                let (args, verb, obj) = match (args, verb, obj) {
                    (Var::List(l), Var::Str(s), Var::Obj(o)) => (l, s, o),
                    (_, _, _) => {
                        self.push(&Var::Err(E_TYPE));
                        return Ok(());
                    }
                };
                // store state variables
                // call verb
                // load state variables
            }
            Op::Return => {}
            Op::Return0 => {}
            Op::Done => {}
            Op::FuncCall { id } => {}
            Op::PushLabel(label) => {
                self.push(&Var::Int(label as i64));
            }
            Op::TryFinally(label) => {
                self.push(&Var::_Finally(label));
            }
            Op::Catch => {
                self.push(&Var::_Catch(1));
            }
            Op::TryExcept(label) => {
                self.push(&Var::_Catch(label));
            }
            Op::EndCatch(label) => {
                let v = self.pop();
                let marker = self.pop();
                let Var::_Catch(marker) = marker else {
                  panic!("Stack marker is not type Catch");
                };
                for i in 0..marker {
                    self.pop();
                }
                self.push(&v);
                self.jump(label);
            }
            Op::EndExcept(label) => {
                let marker = self.pop();
                let Var::_Catch(marker) = marker else {
                    panic!("Stack marker is not type Catch");
                };
                for i in 0..marker {
                    self.pop();
                }
                self.jump(label);
            }
            Op::EndFinally => {
                let v = self.pop();
                let Var::_Finally(marker) = v else {
                    panic!("Stack marker is not type Finally");
                };
                self.push(&Var::Int(FINALLY_FALLTHROUGH));
                self.push(&Var::Int(0));
            }
            Op::Continue(end_label) => {
                unimplemented!("continue")
            }
            Op::Break(end_label) => {
                unimplemented!("break")
            }
            _ => {
                panic!("Unexpected op: {:?} at PC: {}", op, self.top_mut().pc)
            }
        }
        Ok(())
    }
}
