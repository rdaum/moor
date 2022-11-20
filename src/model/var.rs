use crate::model::var::Error::{E_RANGE, E_TYPE};
use decorum::R64;
use int_enum::IntEnum;
use num_traits::identities::Zero;
use serde_derive::{Deserialize, Serialize};
use std::ops::{Mul, Div, Add, Sub, Neg};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Objid(pub i64);

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Ord, PartialOrd, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum Error {
    E_TYPE = 0,
    E_DIV = 1,
    E_PERM = 2,
    E_PROPNF = 3,
    E_VERBNF = 4,
    E_VARNF = 5,
    E_INVIND = 6,
    E_RECMOVE = 7,
    E_MAXREC = 8,
    E_RANGE = 9,
    E_ARGS = 10,
    E_NACC = 11,
    E_INVARG = 12,
    E_QUOTA = 13,
    E_FLOAT = 14,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum VarType {
    TYPE_INT = 0,
    TYPE_OBJ = 1,
    TYPE_STR = 2,
    TYPE_ERR = 3,
    TYPE_LIST = 4,    /* user-visible */
    TYPE_CLEAR = 5,   /* in clear properties' value slot */
    TYPE_NONE = 6,    /* in uninitialized MOO variables */
    TYPE_CATCH = 7,   /* on-stack marker for an exception handler */
    TYPE_FINALLY = 8, /* on-stack marker for a TRY-FINALLY clause */
    TYPE_FLOAT = 9,   /* floating-point number; user-visible */
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Var {
    Clear,
    None,
    Str(String),
    Obj(Objid),
    Int(i64),
    Float(R64),
    Err(Error),
    List(Vec<Var>),

    // Special for parse
    _Catch(usize),
    _Finally(usize),
}

macro_rules! binary_numeric_coercion_op {
    ($op:tt ) => {
        pub fn $op(&self, v: &Var) -> Var {
            match (self, v) {
                (Var::Int(l), Var::Int(r)) => Var::Int(l.$op(r)),
                (Var::Float(l), Var::Int(r)) => Var::Float(l.$op(*r as f64)),
                (Var::Int(l), Var::Float(r)) => {
                    let l = R64::from(*l as f64);
                    Var::Float(l.$op(*r))
                }
                (_, _) => Var::Err(E_TYPE),
            }
        }
    };
}

impl Var {
    pub fn is_true(&self) -> bool {
        match self {
            Var::Str(s) => s != "",
            Var::Int(i) => *i != 0,
            Var::Float(f) => !f.is_zero(),
            Var::List(l) => !l.is_empty(),
            _ => false,
        }
    }

    pub fn has_member(&self, v: &Var) -> Var {
        let Var::List(l) = self else {
            return Var::Err(E_TYPE);
        };

        Var::Int(if l.contains(v) { 1 } else { 0 })
    }

    binary_numeric_coercion_op!(mul);
    binary_numeric_coercion_op!(div);
    binary_numeric_coercion_op!(sub);

    pub fn add(&self, v:&Var) -> Var {
        match (self, v) {
            (Var::Int(l), Var::Int(r)) => Var::Int(l + r),
            (Var::Float(l), Var::Int(r)) => {
                Var::Float(*l + (*r as f64))
            },
            (Var::Int(l), Var::Float(r)) => {
                let l = R64::from(*l as f64);
                Var::Float(l + (*r))
            }
            (Var::Str(s), Var::Str(r)) => {
                let mut c = s.clone();
                c.push_str(r);
                Var::Str(c)
            }
            (_, _) => Var::Err(E_TYPE),
        }
    }

    pub fn modulus(&self, v:&Var) -> Var {
        match (self, v) {
            (Var::Int(l), Var::Int(r)) => Var::Int(l % r),
            (Var::Float(l), Var::Int(r)) => {
                Var::Float(*l % (*r as f64))
            },
            (Var::Int(l), Var::Float(r)) => {
                let l = R64::from(*l as f64);
                Var::Float(l % (*r))
            }
            (_, _) => Var::Err(E_TYPE),
        }
    }

    pub fn negative(&self) -> Var {
        match self {
            Var::Int(l) => Var::Int(-*l),
            Var::Float(f) => Var::Float(f.neg()),
            _ => {
                Var::Err(E_TYPE)
            }
        }
    }

    pub fn index(&self, idx:usize) -> Var {
        match self {
            Var::List(l) => {
                match l.get(idx) {
                    None => Var::Err(E_RANGE),
                    Some(v) => {
                        v.clone()
                    }
                }
            }
            Var::Str(s) => {
                match s.get(idx..idx) {
                    None =>Var::Err(E_RANGE),
                    Some(v) => {
                        Var::Str(String::from(v))
                    }
                }
            }
            _ => Var::Err(E_TYPE)
        }
    }
}
