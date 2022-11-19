use anyhow::anyhow;
use crate::model::var::{Objid, Var};
use crate::model::verbs::Program;
use crate::vm::opcode::{Binary, Op};

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

    pub fn pop(&mut self) -> Var {
        self.rt_stack.pop().unwrap()
    }

    pub fn push(&mut self, v: Var) {
        self.rt_stack.push(v)
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
        $act.push(Var::Int(result))
    };
}
impl VM {
    pub fn exec(&mut self) -> Result<(), anyhow::Error> {
        let mut top_a = match self.stack.last_mut() {
            None => return Err(anyhow!("No current activation")),
            Some(a) => a,
        };
        let op = top_a.next_op();
        match op {
            Op::If => {}
            Op::Jump { label } => {}
            Op::ForList { label, id } => {}
            Op::ForRange { label, id } => {}
            Op::Pop => {}
            Op::Imm => {}
            Op::MkEmptyList => {}
            Op::ListAddTail => {}
            Op::ListAppend => {}
            Op::IndexSet => {}
            Op::MakeSingletonList => {}
            Op::CheckListForSplice => {}
            Op::PutTemp => {}
            Op::PushTemp => {}
            Op::Eq => {
                binary_bool_op!(top_a, ==);
            }
            Op::Ne => {
                binary_bool_op!(top_a, !=);
            }
            Op::Gt => {
                binary_bool_op!(top_a, >);
            }
            Op::Lt => {
                binary_bool_op!(top_a, <);
            }
            Op::Ge => {
                binary_bool_op!(top_a, >=);
            }
            Op::Le => {
                binary_bool_op!(top_a, <=);
            }
            Op::In => {}
            Op::Mul => {
            }
            Op::Sub => {}
            Op::Div => {}
            Op::Mod => {}
            Op::Add => {}
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
        }
        Ok(())
    }
}
