use crate::model::var::{Error, Objid, Var};
use crate::model::verbs::Program;
use crate::vm::opcode::{Binary, Op};
use anyhow::anyhow;
use itertools::Itertools;

struct Activation {
    binary: Binary,
    rt_env: Vec<Var>,
    rt_stack: Vec<Var>,
    pc: usize,
    error_pc: usize,

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
        // I believe this takes a copy. That's ok in this case though.
        let binary = rkyv::from_bytes::<Binary>(&program.0[..]);
        let Ok(binary) = binary else {
            return Err(anyhow!("Invalid opcodes in binary program stream"));
        };

        let rt_env = vec![Var::None; binary.var_names.len()];
        Ok(Activation {
            binary,
            rt_env,
            rt_stack: vec![],
            pc: 0,
            error_pc: 0,
            this,
            player,
            verb_owner,
            definer,
            verb,
            verb_names,
        })
    }

    pub fn next_op(&mut self) -> Op {
        let op = self.binary.main_vector[self.pc].clone();
        self.pc += 1;
        op
    }

    pub fn pop(&mut self) -> Option<Var> {
        self.rt_stack.pop()
    }

    pub fn push(&mut self, v: Var) {
        self.rt_stack.push(v)
    }

    pub fn peek(&self, width: usize) -> Vec<Var> {
        let l = self.rt_stack.len();
        Vec::from(&self.rt_stack[l - width..])
    }

    pub fn poke(&mut self, p: usize, v: &Var) {
        let l = self.rt_stack.len();
        self.rt_stack[l - p] = v.clone()
    }

    pub fn jump(&mut self, label: usize) {
        self.pc += label;
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

    fn next_op(&mut self) -> Op {
        self.top_mut().next_op()
    }

    fn jump(&mut self, label: usize) {
        self.top_mut().jump(label)
    }

    fn set_env(&mut self, id: usize, v: &Var) {
        self.top_mut().rt_env[id] = v.clone();
    }

    fn rewind(&mut self, amt: usize) {
        self.top_mut().rewind(amt);
    }

    fn peek(&self, amt: usize) -> Vec<Var> {
        self.top().peek(amt)
    }

    fn poke(&mut self, pos: usize, v: &Var) {
        self.top_mut().poke(pos, v);
    }

    pub fn exec(&mut self) -> Result<(), anyhow::Error> {
        let op = self.next_op();
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
            Op::ForList { label, id } => {
                let peek = self.peek(2);
                let (count, list) = (&peek[0], &peek[1]);
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
            Op::ForRange { label, id } => {}
            Op::Pop => {
                self.pop();
            }
            Op::Imm => {}
            Op::MkEmptyList => self.push(&Var::List(vec![])),
            Op::ListAddTail => {}
            Op::ListAppend => {}
            Op::IndexSet => {}
            Op::MakeSingletonList => {}
            Op::CheckListForSplice => {}
            Op::PutTemp => {}
            Op::PushTemp => {}
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
            Op::Mod => {

            }
            Op::And => {}
            Op::Or => {}
            Op::Not => {}
            Op::UnaryMinus => {}
            Op::Ref => {}
            Op::PushRef => {}
            Op::RangeRef => {}
            Op::GPut { id } => {}
            Op::GPush { id } => {}
            Op::GetProp => {}
            Op::PushGetProp => {}
            Op::PutProp => {}
            Op::Fork { id, f_index } => {}
            Op::CallVerb => {}
            Op::Return => {}
            Op::Return0 => {}
            Op::Done => {}
            Op::FuncCall { id } => {}
            Op::Length { id } => {}
            Op::Exp => {}
            Op::Scatter {
                done,
                nargs,
                nreg,
                rest,
            } => {}
            Op::PushLabel => {}
            Op::TryFinally => {}
            Op::Catch => {}
            Op::TryExcept => {}
            Op::EndCatch => {}
            Op::EndExcept => {}
            Op::EndFinally => {}
            Op::Continue => {}
            Op::WhileId { id } => {}
            Op::ExitId { id } => {}
            Op::Exit => {}
            _ => {
                panic!("Unexpected op: {:?} at PC: {}", op, self.top_mut().pc)
            }
        }
        Ok(())
    }
}
