use anyhow::anyhow;
use rkyv::{Archive, Deserialize, Serialize};
use bytecheck::CheckBytes;
use rkyv::vec::ArchivedVec;
use crate::model::var::{Objid, Var};
use crate::model::verbs::Program;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
pub enum Op {
    If,
    Jump{label:usize},
    ForList{id: usize, label:usize},
    ForRange{id:usize, label:usize},
    Pop,
    Imm,
    MkEmptyList,
    ListAddTail,
    ListAppend,
    IndexSet,
    MakeSingletonList,
    CheckListForSplice,
    PutTemp,
    PushTemp,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    In,
    Mul,
    Sub,
    Div,
    Mod,
    Add,
    And,
    Or,
    Not,
    UnaryMinus,
    Ref,
    PushRef,
    RangeRef,
    GPut{id:usize},
    GPush{id:usize},
    GetProp,
    PushGetProp,
    PutProp,
    Fork{f_index:usize, id:Option<usize>},
    CallVerb,
    Return,
    Return0,
    Done,
    FuncCall{id:usize},

    // extended
    Length{id:usize},
    Exp,
    Scatter{nargs:usize,nreg:usize,rest:usize,done:usize},
    PushLabel,
    TryFinally,
    Catch,
    TryExcept,
    EndCatch,
    EndExcept,
    EndFinally,
    Continue,
    WhileId{id:usize},
    ExitId{id:usize},
    Exit,
}

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes, Debug))]
struct Binary {
    first_lineno : usize,
    ref_count : usize,
    num_literals: usize,
    var_names : Vec<String>,
    main_vector : Vec<Op>,
}

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
    pub fn new(program: &Program, this:Objid, player:Objid, verb_owner:Objid, definer : Objid, verb: String, verb_names:Vec<String>) -> Result<Self, anyhow::Error> {
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
            verb_names
        })
    }
}

struct VM {

    // Activation stack.
    stack : Vec<Activation>
}