use symbol_table::Symbol;
use int_enum::IntEnum;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Objid(pub i64);

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
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

pub enum Var {
    Clear,
    None,
    Str(String),
    Obj(Objid),
    Int(i64),
    Float(f64),
    Err(Error),
    List(Vec<Var>),

    // Special for parse
    _Catch(usize),
    _Finally(usize),
}

