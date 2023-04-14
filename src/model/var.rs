#![allow(non_camel_case_types, non_snake_case)]

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::{Div, Mul, Neg, Sub};

use decorum::R64;
use int_enum::IntEnum;
use num_traits::identities::Zero;
use rkyv::{Archive, Archived, Deserialize, Serialize};

use crate::compiler::labels::Label;

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Archive,
)]
#[archive(compare(PartialEq), check_bytes)]
pub struct Objid(pub i64);

pub const SYSTEM_OBJECT: Objid = Objid(0);
pub const NOTHING: Objid = Objid(-1);
pub const AMBIGUOUS: Objid = Objid(-2);
pub const FAILED_MATCH: Objid = Objid(-3);

#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    IntEnum,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    Hash,
    Archive,
)]
#[archive(compare(PartialEq), check_bytes)]
#[allow(non_camel_case_types)]
pub enum Error {
    E_NONE = 0,
    E_TYPE = 1,
    E_DIV = 2,
    E_PERM = 3,
    E_PROPNF = 4,
    E_VERBNF = 5,
    E_VARNF = 6,
    E_INVIND = 7,
    E_RECMOVE = 8,
    E_MAXREC = 9,
    E_RANGE = 10,
    E_ARGS = 11,
    E_NACC = 12,
    E_INVARG = 13,
    E_QUOTA = 14,
    E_FLOAT = 15,
}

pub struct ErrorPack {
    pub code: Error,
    pub msg: String,
    pub value: Var,
}

impl Error {
    pub fn message(&self) -> &str {
        match self {
            Error::E_NONE => "No error",
            Error::E_TYPE => "Type mismatch",
            Error::E_DIV => "Division by zero",
            Error::E_PERM => "Permission denied",
            Error::E_PROPNF => "Property not found",
            Error::E_VERBNF => "Verb not found",
            Error::E_VARNF => "Variable not found",
            Error::E_INVIND => "Invalid indirection",
            Error::E_RECMOVE => "Recursive move",
            Error::E_MAXREC => "Too many verb calls",
            Error::E_RANGE => "Range error",
            Error::E_ARGS => "Incorrect number of arguments",
            Error::E_NACC => "Move refused by destination",
            Error::E_INVARG => "Invalid argument",
            Error::E_QUOTA => "Resource limit exceeded",
            Error::E_FLOAT => "Floating-point arithmetic error",
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Error::E_NONE => "E_NONE",
            Error::E_TYPE => "E_TYPE",
            Error::E_DIV => "E_DIV",
            Error::E_PERM => "E_PERM",
            Error::E_PROPNF => "E_PROPNF",
            Error::E_VERBNF => "E_VERBNF",
            Error::E_VARNF => "E_VARNF",
            Error::E_INVIND => "E_INVIND",
            Error::E_RECMOVE => "E_RECMOVE",
            Error::E_MAXREC => "E_MAXREC",
            Error::E_RANGE => "E_RANGE",
            Error::E_ARGS => "E_ARGS",
            Error::E_NACC => "E_NACC",
            Error::E_INVARG => "E_INVARG",
            Error::E_QUOTA => "E_QUOTA",
            Error::E_FLOAT => "E_FLOAT",
        }
    }

    pub fn make_raise_pack(&self, msg: String, value: Var) -> ErrorPack {
        ErrorPack {
            code: *self,
            msg,
            value,
        }
    }

    pub fn make_error_pack(&self) -> ErrorPack {
        ErrorPack {
            code: *self,
            msg: self.message().to_string(),
            value: Var::None,
        }
    }
}

pub struct ArchivedUsize(Archived<usize>);

impl PartialEq<usize> for ArchivedUsize {
    fn eq(&self, other: &usize) -> bool {
        self.0 as u64 == *other as u64
    }
}

impl PartialEq<ArchivedUsize> for usize {
    fn eq(&self, other: &ArchivedUsize) -> bool {
        *self as u64 == other.0 as u64
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Serialize, Deserialize, Archive)]
#[archive(compare(PartialEq), check_bytes)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Archive)]
#[archive(compare(PartialEq), check_bytes)]
#[archive(bound(serialize = "__S: rkyv::ser::ScratchSpace + rkyv::ser::Serializer"))]
#[archive_attr(check_bytes(
    bound = "__C: rkyv::validation::ArchiveContext, <__C as rkyv::Fallible>::Error: rkyv::bytecheck::Error"
))]
pub enum Var {
    Clear,
    None,
    Str(String),
    Obj(Objid),
    Int(i64),
    Float(f64),
    Err(Error),
    List(
        #[omit_bounds]
        #[archive_attr(omit_bounds)]
        Vec<Var>,
    ),
    // Special for exception handling
    _Catch(Label),
    _Finally(Label),
    _Label(Label),
}

impl Var {
    pub fn type_id(&self) -> VarType {
        match self {
            Var::Clear => VarType::TYPE_CLEAR,
            Var::None => VarType::TYPE_NONE,
            Var::Str(_) => VarType::TYPE_STR,
            Var::Obj(_) => VarType::TYPE_OBJ,
            Var::Int(_) => VarType::TYPE_INT,
            Var::Float(_) => VarType::TYPE_FLOAT,
            Var::Err(_) => VarType::TYPE_ERR,
            Var::List(_) => VarType::TYPE_LIST,
            Var::_Catch(_) => VarType::TYPE_CATCH,
            Var::_Finally(_) => VarType::TYPE_FINALLY,
            Var::_Label(_) => VarType::TYPE_CATCH,
        }
    }
}
impl PartialEq<Self> for Var {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Var::Clear, Var::Clear) => true,
            (Var::None, Var::None) => true,
            (Var::Str(l), Var::Str(r)) => l == r,
            (Var::Obj(l), Var::Obj(r)) => l == r,
            (Var::Int(l), Var::Int(r)) => l == r,
            (Var::Float(l), Var::Float(r)) => l == r,
            (Var::Err(l), Var::Err(r)) => l == r,
            (Var::List(l), Var::List(r)) => l == r,
            (Var::Clear, _) => false,
            (Var::None, _) => false,
            (Var::Str(_), _) => false,
            (Var::Obj(_), _) => false,
            (Var::Int(_), _) => false,
            (Var::Float(_), _) => false,
            (Var::Err(_), _) => false,
            (Var::List(_), _) => false,
            (Var::_Catch(a), Var::_Catch(b)) => a == b,
            (Var::_Finally(a), Var::_Finally(b)) => a == b,
            (Var::_Label(a), Var::_Label(b)) => a == b,
            (Var::_Catch(_a), _) => false,
            (Var::_Label(_a), _) => false,
            (Var::_Finally(_a), _) => false,
        }
    }
}

impl PartialOrd<Self> for Var {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Var::Clear, Var::Clear) => Some(Ordering::Equal),
            (Var::None, Var::None) => Some(Ordering::Equal),
            (Var::Str(l), Var::Str(r)) => l.partial_cmp(r),
            (Var::Obj(l), Var::Obj(r)) => l.partial_cmp(r),
            (Var::Int(l), Var::Int(r)) => l.partial_cmp(r),
            (Var::Float(l), Var::Float(r)) => R64::from(*l).partial_cmp(&R64::from(*r)),
            (Var::Err(l), Var::Err(r)) => l.partial_cmp(r),
            (Var::List(l), Var::List(r)) => l.partial_cmp(r),
            (Var::Clear, _) => Some(Ordering::Less),
            (Var::None, _) => Some(Ordering::Less),
            (Var::Str(_), _) => Some(Ordering::Less),
            (Var::Obj(_), _) => Some(Ordering::Less),
            (Var::Int(_), _) => Some(Ordering::Less),
            (Var::Float(_), _) => Some(Ordering::Less),
            (Var::Err(_), _) => Some(Ordering::Less),
            (Var::List(_), _) => Some(Ordering::Less),
            (Var::_Catch(a), Var::_Catch(b)) => a.partial_cmp(b),
            (Var::_Finally(a), Var::_Finally(b)) => a.partial_cmp(b),
            (Var::_Label(a), Var::_Label(b)) => a.partial_cmp(b),
            (Var::_Catch(_a), _) => Some(Ordering::Less),
            (Var::_Label(_a), _) => Some(Ordering::Less),
            (Var::_Finally(_a), _) => Some(Ordering::Less),
        }
    }
}

impl Ord for Var {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Var::Clear, Var::Clear) => Ordering::Equal,
            (Var::None, Var::None) => Ordering::Equal,
            (Var::Str(l), Var::Str(r)) => l.cmp(r),
            (Var::Obj(l), Var::Obj(r)) => l.cmp(r),
            (Var::Int(l), Var::Int(r)) => l.cmp(r),
            (Var::Float(l), Var::Float(r)) => R64::from(*l).cmp(&R64::from(*r)),
            (Var::Err(l), Var::Err(r)) => l.cmp(r),
            (Var::List(l), Var::List(r)) => l.cmp(r),
            (Var::Clear, _) => Ordering::Less,
            (Var::None, _) => Ordering::Less,
            (Var::Str(_), _) => Ordering::Less,
            (Var::Obj(_), _) => Ordering::Less,
            (Var::Int(_), _) => Ordering::Less,
            (Var::Float(_), _) => Ordering::Less,
            (Var::Err(_), _) => Ordering::Less,
            (Var::List(_), _) => Ordering::Less,
            (Var::_Catch(a), Var::_Catch(b)) => a.cmp(b),
            (Var::_Finally(a), Var::_Finally(b)) => a.cmp(b),
            (Var::_Label(a), Var::_Label(b)) => a.cmp(b),
            (Var::_Catch(_a), _) => Ordering::Less,
            (Var::_Label(_a), _) => Ordering::Less,
            (Var::_Finally(_a), _) => Ordering::Less,
        }
    }
}
impl Hash for Var {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let t = self.type_id() as u8;
        t.hash(state);
        match self {
            Var::Clear => {}
            Var::None => {}
            Var::Str(s) => s.hash(state),
            Var::Obj(o) => o.hash(state),
            Var::Int(i) => i.hash(state),
            Var::Float(f) => R64::from(*f).hash(state),
            Var::Err(e) => e.hash(state),
            Var::List(l) => l.hash(state),
            Var::_Catch(l) => l.hash(state),
            Var::_Finally(l) => l.hash(state),
            Var::_Label(l) => l.hash(state),
        }
    }
}
impl Eq for Var {}

macro_rules! binary_numeric_coercion_op {
    ($op:tt ) => {
        pub fn $op(&self, v: &Var) -> Result<Var, Error> {
            match (self, v) {
                (Var::Float(l), Var::Float(r)) => Ok(Var::Float(l.$op(*r))),
                (Var::Int(l), Var::Int(r)) => Ok(Var::Int(l.$op(*r))),
                (Var::Float(l), Var::Int(r)) => Ok(Var::Float(l.$op(*r as f64))),
                (Var::Int(l), Var::Float(r)) => Ok(Var::Float((*l as f64).$op(*r))),
                (_, _) => Err(Error::E_TYPE),
            }
        }
    };
}

impl Var {
    pub fn is_true(&self) -> bool {
        match self {
            Var::Str(s) => !s.is_empty(),
            Var::Int(i) => *i != 0,
            Var::Float(f) => !f.is_zero(),
            Var::List(l) => !l.is_empty(),
            _ => false,
        }
    }

    pub fn has_member(&self, v: &Var) -> Var {
        let Var::List(l) = self else {
            return Var::Err(Error::E_TYPE);
        };

        Var::Int(if l.contains(v) { 1 } else { 0 })
    }

    binary_numeric_coercion_op!(mul);
    binary_numeric_coercion_op!(div);
    binary_numeric_coercion_op!(sub);

    pub fn add(&self, v: &Var) -> Result<Var, Error> {
        match (self, v) {
            (Var::Float(l), Var::Float(r)) => Ok(Var::Float(*l + *r)),
            (Var::Int(l), Var::Int(r)) => Ok(Var::Int(l + r)),
            (Var::Float(l), Var::Int(r)) => Ok(Var::Float(*l + (*r as f64))),
            (Var::Int(l), Var::Float(r)) => Ok(Var::Float(*l as f64 + *r)),
            (Var::Str(s), Var::Str(r)) => {
                let mut c = s.clone();
                c.push_str(r);
                Ok(Var::Str(c))
            }
            (_, _) => Err(Error::E_TYPE),
        }
    }

    pub fn negative(&self) -> Result<Var, Error> {
        match self {
            Var::Int(l) => Ok(Var::Int(-*l)),
            Var::Float(f) => Ok(Var::Float(f.neg())),
            _ => Err(Error::E_TYPE),
        }
    }

    pub fn modulus(&self, v: &Var) -> Result<Var, Error> {
        match (self, v) {
            (Var::Float(l), Var::Float(r)) => Ok(Var::Float(*l % *r)),
            (Var::Int(l), Var::Int(r)) => Ok(Var::Int(l % r)),
            (Var::Float(l), Var::Int(r)) => Ok(Var::Float(*l % (*r as f64))),
            (Var::Int(l), Var::Float(r)) => Ok(Var::Float(*l as f64 % (*r))),
            (_, _) => Err(Error::E_TYPE),
        }
    }

    pub fn pow(&self, v: &Var) -> Result<Var, Error> {
        match (self, v) {
            (Var::Float(l), Var::Float(r)) => Ok(Var::Float(l.powf(*r))),
            (Var::Int(l), Var::Int(r)) => Ok(Var::Int(l.pow(*r as u32))),
            (Var::Float(l), Var::Int(r)) => Ok(Var::Float(l.powi(*r as i32))),
            (Var::Int(l), Var::Float(r)) => Ok(Var::Float((*l as f64).powf(*r))),
            (_, _) => Err(Error::E_TYPE),
        }
    }

    pub fn index(&self, idx: usize) -> Result<Var, Error> {
        match self {
            Var::List(l) => match l.get(idx) {
                None => Err(Error::E_RANGE),
                Some(v) => Ok(v.clone()),
            },
            Var::Str(s) => match s.get(idx..idx + 1) {
                None => Err(Error::E_RANGE),
                Some(v) => Ok(Var::Str(String::from(v))),
            },
            _ => Err(Error::E_TYPE),
        }
    }

    pub fn range(&self, from: i64, to: i64) -> Result<Var, Error> {
        match self {
            Var::Str(s) => {
                let len = s.len() as i64;
                if from <= 0 || from > len + 1 || to < 1 || to > len {
                    return Err(Error::E_RANGE);
                }
                let (from, to) = (from as usize, to as usize);
                Ok(Var::Str(s[from - 1..to].to_string()))
            }
            Var::List(l) => {
                let len = l.len() as i64;
                if from <= 0 || from > len + 1 || to < 1 || to > len {
                    return Err(Error::E_RANGE);
                }
                let (from, to) = (from as usize, to as usize);
                let mut res = Vec::with_capacity(to - from + 1);
                for i in from..=to {
                    res.push(l[i - 1].clone());
                }
                Ok(Var::List(res))
            }
            _ => Err(Error::E_TYPE),
        }
    }

    pub fn rangeset(&self, value: Var, from: i64, to: i64) -> Result<Var, Error> {
        let (base_len, val_len) = match (self, &value) {
            (Var::Str(base_str), Var::Str(val_str)) => {
                (base_str.len() as i64, val_str.len() as i64)
            }
            (Var::List(base_list), Var::List(val_list)) => {
                (base_list.len() as i64, val_list.len() as i64)
            }
            _ => return Err(Error::E_TYPE),
        };

        if from <= 0 || from > base_len + 1 || to < 1 || to > base_len {
            return Err(Error::E_RANGE);
        }

        let lenleft = if from > 1 { from - 1 } else { 0 };
        let lenmiddle = val_len;
        let lenright = if base_len > to { base_len - to } else { 0 };
        let newsize = lenleft + lenmiddle + lenright;

        let (from, to) = (from as usize, to as usize);
        let ans = match (self, &value) {
            (Var::Str(base_str), Var::Str(value_str)) => {
                let mut ans = String::with_capacity(newsize as usize);
                ans.push_str(&base_str[..from - 1]);
                ans.push_str(value_str);
                ans.push_str(&base_str[to..]);
                Var::Str(ans)
            }
            (Var::List(base_list), Var::List(value_list)) => {
                let mut ans: Vec<Var> = Vec::with_capacity(newsize as usize);
                ans.extend_from_slice(&base_list[..from - 1]);
                ans.extend(value_list.iter().cloned());
                ans.extend_from_slice(&base_list[to..]);
                Var::List(ans)
            }
            _ => unreachable!(),
        };

        Ok(ans)
    }
}

impl<'a> From<&'a str> for Var {
    fn from(s: &'a str) -> Self {
        Self::Str(s.to_string())
    }
}

impl From<String> for Var {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<i64> for Var {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

impl From<f64> for Var {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<Objid> for Var {
    fn from(o: Objid) -> Self {
        Self::Obj(o)
    }
}

impl From<Vec<Var>> for Var {
    fn from(l: Vec<Var>) -> Self {
        Self::List(l)
    }
}

impl From<Error> for Var {
    fn from(e: Error) -> Self {
        Self::Err(e)
    }
}

fn rangeset_check(base: &Var, _inst: &Var, from: i64, to: i64) -> Result<(), Error> {
    let blen = match base {
        Var::Str(s) => s.len() as i64,
        Var::List(l) => l.len() as i64,
        _ => return Err(Error::E_TYPE),
    };

    if from <= 0 || from > blen + 1 || to < 1 || to > blen {
        return Err(Error::E_RANGE);
    }

    Ok(())
}

fn listrangeset(base: Var, from: i64, to: i64, value: Var) -> Result<Var, Error> {
    rangeset_check(&base, &value, from, to)?;

    let (base_list, val_list) = match (base, value) {
        (Var::List(bl), Var::List(vl)) => (bl, vl),
        _ => return Err(Error::E_TYPE),
    };

    let val_len = val_list.len() as i64;
    let base_len = base_list.len() as i64;
    let lenleft = if from > 1 { from - 1 } else { 0 };
    let lenmiddle = val_len;
    let lenright = if base_len > to { base_len - to } else { 0 };
    let newsize = lenleft + lenmiddle + lenright;

    let mut ans = Vec::with_capacity(newsize as usize);

    let (from, to) = (from as usize, to as usize);
    ans.extend_from_slice(&base_list[..from - 1]);
    ans.extend_from_slice(&val_list);
    ans.extend_from_slice(&base_list[to..]);

    Ok(Var::List(ans))
}

fn strrangeset(base: Var, from: i64, to: i64, value: Var) -> Result<Var, Error> {
    rangeset_check(&base, &value, from, to)?;

    let (base_str, val_str) = match (base, value) {
        (Var::Str(base), Var::Str(val)) => (base, val),
        _ => return Err(Error::E_TYPE),
    };

    let base_len = base_str.len() as i64;
    let val_len = val_str.len() as i64;
    let lenleft = if from > 1 { from - 1 } else { 0 };
    let lenmiddle = val_len;
    let lenright = if base_len > to { base_len - to } else { 0 };
    let newsize = lenleft + lenmiddle + lenright;

    let (from, to) = (from as usize, to as usize);
    let mut s = String::with_capacity(newsize as usize);
    s.push_str(&base_str[..(from - 1)]);
    s.push_str(&val_str);
    s.push_str(&base_str[to..]);

    Ok(Var::Str(s))
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(Var::Int(1).add(&Var::Int(2)), Ok(Var::Int(3)));
        assert_eq!(Var::Int(1).add(&Var::Float(2.0)), Ok(Var::Float(3.0)));
        assert_eq!(Var::Float(1.).add(&Var::Int(2)), Ok(Var::Float(3.)));
        assert_eq!(Var::Float(1.).add(&Var::Float(2.)), Ok(Var::Float(3.)));
        assert_eq!(
            Var::Str(String::from("a")).add(&Var::Str(String::from("b"))),
            Ok(Var::Str(String::from("ab")))
        );
    }

    #[test]
    fn test_sub() -> Result<(), Error> {
        assert_eq!(Var::Int(1).sub(&Var::Int(2))?, Var::Int(-1));
        assert_eq!(Var::Int(1).sub(&Var::Float(2.))?, Var::Float(-1.));
        assert_eq!(Var::Float(1.).sub(&Var::Int(2))?, Var::Float(-1.));
        assert_eq!(Var::Float(1.).sub(&Var::Float(2.))?, Var::Float(-1.));
        Ok(())
    }

    #[test]
    fn test_mul() -> Result<(), Error> {
        assert_eq!(Var::Int(1).mul(&Var::Int(2))?, Var::Int(2));
        assert_eq!(Var::Int(1).mul(&Var::Float(2.))?, Var::Float(2.));
        assert_eq!(Var::Float(1.).mul(&Var::Int(2))?, Var::Float(2.));
        assert_eq!(Var::Float(1.).mul(&Var::Float(2.))?, Var::Float(2.));
        Ok(())
    }

    #[test]
    fn test_div() -> Result<(), Error> {
        assert_eq!(Var::Int(1).div(&Var::Int(2))?, Var::Int(0));
        assert_eq!(Var::Int(1).div(&Var::Float(2.))?, Var::Float(0.5));
        assert_eq!(Var::Float(1.).div(&Var::Int(2))?, Var::Float(0.5));
        assert_eq!(Var::Float(1.).div(&Var::Float(2.))?, Var::Float(0.5));
        Ok(())
    }

    #[test]
    fn test_modulus() {
        assert_eq!(Var::Int(1).modulus(&Var::Int(2)), Ok(Var::Int(1)));
        assert_eq!(Var::Int(1).modulus(&Var::Float(2.)), Ok(Var::Float(1.)));
        assert_eq!(Var::Float(1.).modulus(&Var::Int(2)), Ok(Var::Float(1.)));
        assert_eq!(Var::Float(1.).modulus(&Var::Float(2.)), Ok(Var::Float(1.)));
        assert_eq!(
            Var::Str("moop".into()).modulus(&Var::Int(2)),
            Err(Error::E_TYPE)
        );
    }

    #[test]
    fn test_pow() {
        assert_eq!(Var::Int(1).pow(&Var::Int(2)), Ok(Var::Int(1)));
        assert_eq!(Var::Int(2).pow(&Var::Int(2)), Ok(Var::Int(4)));
        assert_eq!(Var::Int(2).pow(&Var::Float(2.)), Ok(Var::Float(4.)));
        assert_eq!(Var::Float(2.).pow(&Var::Int(2)), Ok(Var::Float(4.)));
        assert_eq!(Var::Float(2.).pow(&Var::Float(2.)), Ok(Var::Float(4.)));
    }

    #[test]
    fn test_negative() {
        assert_eq!(Var::Int(1).negative(), Ok(Var::Int(-1)));
        assert_eq!(Var::Float(1.).negative(), Ok(Var::Float(-1.0)));
    }

    #[test]
    fn test_index() {
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)]).index(0),
            Ok(Var::Int(1))
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)]).index(1),
            Ok(Var::Int(2))
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)]).index(2),
            Err(Error::E_RANGE)
        );
        assert_eq!(
            Var::Str(String::from("ab")).index(0),
            Ok(Var::Str(String::from("a")))
        );
        assert_eq!(
            Var::Str(String::from("ab")).index(1),
            Ok(Var::Str(String::from("b")))
        );
        assert_eq!(Var::Str(String::from("ab")).index(2), Err(Error::E_RANGE));
    }

    #[test]
    fn test_eq() {
        assert_eq!(Var::Int(1), Var::Int(1));
        assert_eq!(Var::Float(1.), Var::Float(1.));
        assert_eq!(Var::Str(String::from("a")), Var::Str(String::from("a")));
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)]),
            Var::List(vec![Var::Int(1), Var::Int(2)])
        );
        assert_eq!(Var::Obj(Objid(1)), Var::Obj(Objid(1)));
        assert_eq!(Var::Err(Error::E_TYPE), Var::Err(Error::E_TYPE));
    }

    #[test]
    fn test_ne() {
        assert_ne!(Var::Int(1), Var::Int(2));
        assert_ne!(Var::Float(1.), Var::Float(2.));
        assert_ne!(Var::Str(String::from("a")), Var::Str(String::from("b")));
        assert_ne!(
            Var::List(vec![Var::Int(1), Var::Int(2)]),
            Var::List(vec![Var::Int(1), Var::Int(3)])
        );
        assert_ne!(Var::Obj(Objid(1)), Var::Obj(Objid(2)));
        assert_ne!(Var::Err(Error::E_TYPE), Var::Err(Error::E_RANGE));
    }

    #[test]
    fn test_lt() {
        assert!(Var::Int(1) < Var::Int(2));
        assert!(Var::Float(1.) < Var::Float(2.));
        assert!(Var::Str(String::from("a")) < Var::Str(String::from("b")));
        assert!(
            Var::List(vec![Var::Int(1), Var::Int(2)]) < Var::List(vec![Var::Int(1), Var::Int(3)])
        );
        assert!(Var::Obj(Objid(1)) < Var::Obj(Objid(2)));
        assert!(Var::Err(Error::E_TYPE) < Var::Err(Error::E_RANGE));
    }

    #[test]
    fn test_le() {
        assert!(Var::Int(1) <= Var::Int(2));
        assert!(Var::Float(1.) <= Var::Float(2.));
        assert!(Var::Str(String::from("a")) <= Var::Str(String::from("b")));
        assert!(
            Var::List(vec![Var::Int(1), Var::Int(2)]) <= Var::List(vec![Var::Int(1), Var::Int(3)])
        );
        assert!(Var::Obj(Objid(1)) <= Var::Obj(Objid(2)));
        assert!(Var::Err(Error::E_TYPE) <= Var::Err(Error::E_RANGE));
    }

    #[test]
    fn test_gt() {
        assert!(Var::Int(2) > Var::Int(1));
        assert!(Var::Float(2.) > Var::Float(1.));
        assert!(Var::Str(String::from("b")) > Var::Str(String::from("a")));
        assert!(
            Var::List(vec![Var::Int(1), Var::Int(3)]) > Var::List(vec![Var::Int(1), Var::Int(2)])
        );
        assert!(Var::Obj(Objid(2)) > Var::Obj(Objid(1)));
        assert!(Var::Err(Error::E_RANGE) > Var::Err(Error::E_TYPE));
    }

    #[test]
    fn test_ge() {
        assert!(Var::Int(2) >= Var::Int(1));
        assert!(Var::Float(2.) >= Var::Float(1.));
        assert!(Var::Str(String::from("b")) >= Var::Str(String::from("a")));
        assert!(
            Var::List(vec![Var::Int(1), Var::Int(3)]) >= Var::List(vec![Var::Int(1), Var::Int(2)])
        );
        assert!(Var::Obj(Objid(2)) >= Var::Obj(Objid(1)));
        assert!(Var::Err(Error::E_RANGE) >= Var::Err(Error::E_TYPE));
    }

    #[test]
    fn test_partial_cmp() {
        assert_eq!(Var::Int(1).partial_cmp(&Var::Int(1)), Some(Ordering::Equal));
        assert_eq!(
            Var::Float(1.).partial_cmp(&Var::Float(1.)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Var::Str(String::from("a")).partial_cmp(&Var::Str(String::from("a"))),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)])
                .partial_cmp(&Var::List(vec![Var::Int(1), Var::Int(2)])),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Var::Obj(Objid(1)).partial_cmp(&Var::Obj(Objid(1))),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Var::Err(Error::E_TYPE).partial_cmp(&Var::Err(Error::E_TYPE)),
            Some(Ordering::Equal)
        );

        assert_eq!(Var::Int(1).partial_cmp(&Var::Int(2)), Some(Ordering::Less));
        assert_eq!(
            Var::Float(1.).partial_cmp(&Var::Float(2.)),
            Some(Ordering::Less)
        );
        assert_eq!(
            Var::Str(String::from("a")).partial_cmp(&Var::Str(String::from("b"))),
            Some(Ordering::Less)
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)])
                .partial_cmp(&Var::List(vec![Var::Int(1), Var::Int(3)])),
            Some(Ordering::Less)
        );
        assert_eq!(
            Var::Obj(Objid(1)).partial_cmp(&Var::Obj(Objid(2))),
            Some(Ordering::Less)
        );
        assert_eq!(
            Var::Err(Error::E_TYPE).partial_cmp(&Var::Err(Error::E_RANGE)),
            Some(Ordering::Less)
        );

        assert_eq!(
            Var::Int(2).partial_cmp(&Var::Int(1)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Var::Float(2.).partial_cmp(&Var::Float(1.)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Var::Str(String::from("b")).partial_cmp(&Var::Str(String::from("a"))),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(3)])
                .partial_cmp(&Var::List(vec![Var::Int(1), Var::Int(2)])),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Var::Obj(Objid(2)).partial_cmp(&Var::Obj(Objid(1))),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Var::Err(Error::E_RANGE).partial_cmp(&Var::Err(Error::E_TYPE)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn test_cmp() {
        assert_eq!(Var::Int(1).cmp(&Var::Int(1)), Ordering::Equal);
        assert_eq!(Var::Float(1.).cmp(&Var::Float(1.)), Ordering::Equal);
        assert_eq!(
            Var::Str(String::from("a")).cmp(&Var::Str(String::from("a"))),
            Ordering::Equal
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)])
                .cmp(&Var::List(vec![Var::Int(1), Var::Int(2)])),
            Ordering::Equal
        );
        assert_eq!(Var::Obj(Objid(1)).cmp(&Var::Obj(Objid(1))), Ordering::Equal);
        assert_eq!(
            Var::Err(Error::E_TYPE).cmp(&Var::Err(Error::E_TYPE)),
            Ordering::Equal
        );

        assert_eq!(Var::Int(1).cmp(&Var::Int(2)), Ordering::Less);
        assert_eq!(Var::Float(1.).cmp(&Var::Float(2.)), Ordering::Less);
        assert_eq!(
            Var::Str(String::from("a")).cmp(&Var::Str(String::from("b"))),
            Ordering::Less
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)])
                .cmp(&Var::List(vec![Var::Int(1), Var::Int(3)])),
            Ordering::Less
        );
        assert_eq!(Var::Obj(Objid(1)).cmp(&Var::Obj(Objid(2))), Ordering::Less);
        assert_eq!(
            Var::Err(Error::E_TYPE).cmp(&Var::Err(Error::E_RANGE)),
            Ordering::Less
        );

        assert_eq!(Var::Int(2).cmp(&Var::Int(1)), Ordering::Greater);
        assert_eq!(Var::Float(2.).cmp(&Var::Float(1.)), Ordering::Greater);
        assert_eq!(
            Var::Str(String::from("b")).cmp(&Var::Str(String::from("a"))),
            Ordering::Greater
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(3)])
                .cmp(&Var::List(vec![Var::Int(1), Var::Int(2)])),
            Ordering::Greater
        );
        assert_eq!(
            Var::Obj(Objid(2)).cmp(&Var::Obj(Objid(1))),
            Ordering::Greater
        );
        assert_eq!(
            Var::Err(Error::E_RANGE).cmp(&Var::Err(Error::E_TYPE)),
            Ordering::Greater
        );
    }

    #[test]
    fn test_partial_ord() {
        assert_eq!(
            Var::Int(1).partial_cmp(&Var::Int(1)).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            Var::Float(1.).partial_cmp(&Var::Float(1.)).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            Var::Str(String::from("a"))
                .partial_cmp(&Var::Str(String::from("a")))
                .unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)])
                .partial_cmp(&Var::List(vec![Var::Int(1), Var::Int(2)]))
                .unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            Var::Obj(Objid(1)).partial_cmp(&Var::Obj(Objid(1))).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            Var::Err(Error::E_TYPE)
                .partial_cmp(&Var::Err(Error::E_TYPE))
                .unwrap(),
            Ordering::Equal
        );

        assert_eq!(
            Var::Int(1).partial_cmp(&Var::Int(2)).unwrap(),
            Ordering::Less
        );
        assert_eq!(
            Var::Float(1.).partial_cmp(&Var::Float(2.)).unwrap(),
            Ordering::Less
        );
        assert_eq!(
            Var::Str(String::from("a"))
                .partial_cmp(&Var::Str(String::from("b")))
                .unwrap(),
            Ordering::Less
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)])
                .partial_cmp(&Var::List(vec![Var::Int(1), Var::Int(3)]))
                .unwrap(),
            Ordering::Less
        );
        assert_eq!(
            Var::Obj(Objid(1)).partial_cmp(&Var::Obj(Objid(2))).unwrap(),
            Ordering::Less
        );
        assert_eq!(
            Var::Err(Error::E_TYPE)
                .partial_cmp(&Var::Err(Error::E_RANGE))
                .unwrap(),
            Ordering::Less
        );

        assert_eq!(
            Var::Int(2).partial_cmp(&Var::Int(1)).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            Var::Float(2.).partial_cmp(&Var::Float(1.)).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            Var::Str(String::from("b"))
                .partial_cmp(&Var::Str(String::from("a")))
                .unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(3)])
                .partial_cmp(&Var::List(vec![Var::Int(1), Var::Int(2)]))
                .unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            Var::Obj(Objid(2)).partial_cmp(&Var::Obj(Objid(1))).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            Var::Err(Error::E_RANGE)
                .partial_cmp(&Var::Err(Error::E_TYPE))
                .unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn test_ord() {
        assert_eq!(Var::Int(1).cmp(&Var::Int(1)), Ordering::Equal);
        assert_eq!(Var::Float(1.).cmp(&Var::Float(1.)), Ordering::Equal);
        assert_eq!(
            Var::Str(String::from("a")).cmp(&Var::Str(String::from("a"))),
            Ordering::Equal
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)])
                .cmp(&Var::List(vec![Var::Int(1), Var::Int(2)])),
            Ordering::Equal
        );
        assert_eq!(Var::Obj(Objid(1)).cmp(&Var::Obj(Objid(1))), Ordering::Equal);
        assert_eq!(
            Var::Err(Error::E_TYPE).cmp(&Var::Err(Error::E_TYPE)),
            Ordering::Equal
        );

        assert_eq!(Var::Int(1).cmp(&Var::Int(2)), Ordering::Less);
        assert_eq!(Var::Float(1.).cmp(&Var::Float(2.)), Ordering::Less);
        assert_eq!(
            Var::Str(String::from("a")).cmp(&Var::Str(String::from("b"))),
            Ordering::Less
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)])
                .cmp(&Var::List(vec![Var::Int(1), Var::Int(3)])),
            Ordering::Less
        );
        assert_eq!(Var::Obj(Objid(1)).cmp(&Var::Obj(Objid(2))), Ordering::Less);
        assert_eq!(
            Var::Err(Error::E_TYPE).cmp(&Var::Err(Error::E_RANGE)),
            Ordering::Less
        );

        assert_eq!(Var::Int(2).cmp(&Var::Int(1)), Ordering::Greater);
        assert_eq!(Var::Float(2.).cmp(&Var::Float(1.)), Ordering::Greater);
        assert_eq!(
            Var::Str(String::from("b")).cmp(&Var::Str(String::from("a"))),
            Ordering::Greater
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(3)])
                .cmp(&Var::List(vec![Var::Int(1), Var::Int(2)])),
            Ordering::Greater
        );
        assert_eq!(
            Var::Obj(Objid(2)).cmp(&Var::Obj(Objid(1))),
            Ordering::Greater
        );
        assert_eq!(
            Var::Err(Error::E_RANGE).cmp(&Var::Err(Error::E_TYPE)),
            Ordering::Greater
        );
    }

    #[test]
    fn test_is_true() {
        assert!(Var::Int(1).is_true());
        assert!(Var::Float(1.).is_true());
        assert!(Var::Str(String::from("a")).is_true());
        assert!(Var::List(vec![Var::Int(1), Var::Int(2)]).is_true());
        assert!(!Var::Obj(Objid(1)).is_true());
        assert!(!Var::Err(Error::E_TYPE).is_true());
    }

    #[test]
    fn test_listrangeset() {
        let base = Var::List(vec![Var::Int(1), Var::Int(2), Var::Int(3), Var::Int(4)]);

        // {1,2,3,4}[1..2] = {"a", "b", "c"} => {1, "a", "b", "c", 4}
        let value = Var::List(vec![
            Var::Str("a".to_string()),
            Var::Str("b".to_string()),
            Var::Str("c".to_string()),
        ]);
        let expected = Var::List(vec![
            Var::Int(1),
            Var::Str("a".to_string()),
            Var::Str("b".to_string()),
            Var::Str("c".to_string()),
            Var::Int(4),
        ]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {"a"} => {1, "a", 4}
        let value = Var::List(vec![Var::Str("a".to_string())]);
        let expected = Var::List(vec![Var::Int(1), Var::Str("a".to_string()), Var::Int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {} => {1,4}
        let value = Var::List(vec![]);
        let expected = Var::List(vec![Var::Int(1), Var::Int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {"a", "b"} => {1, "a", "b", 4}
        let value = Var::List(vec![Var::Str("a".to_string()), Var::Str("b".to_string())]);
        let expected = Var::List(vec![
            Var::Int(1),
            Var::Str("a".to_string()),
            Var::Str("b".to_string()),
            Var::Int(4),
        ]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);
    }

    #[test]
    fn test_strrangeset() {
        // Test interior insertion
        let base = Var::Str("12345".to_string());
        let value = Var::Str("abc".to_string());
        let expected = Var::Str("1abc45".to_string());
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior replacement
        let base = Var::Str("12345".to_string());
        let value = Var::Str("ab".to_string());
        let expected = Var::Str("1ab45".to_string());
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior deletion
        let base = Var::Str("12345".to_string());
        let value = Var::Str("".to_string());
        let expected = Var::Str("145".to_string());
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior subtraction
        let base = Var::Str("12345".to_string());
        let value = Var::Str("z".to_string());
        let expected = Var::Str("1z45".to_string());
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_rangeset_check_negative() {
        // Test negative cases for strings
        let base = Var::Str("abcdef".to_string());
        let instr = Var::Str("ghi".to_string());
        assert_eq!(base.rangeset(instr.clone(), 1, 0), Err(Error::E_RANGE));
        assert_eq!(base.rangeset(instr.clone(), 0, 3), Err(Error::E_RANGE));
        assert_eq!(base.rangeset(instr.clone(), 2, 7), Err(Error::E_RANGE));
        assert_eq!(base.rangeset(instr, 1, 100), Err(Error::E_RANGE));

        // Test negative cases for lists
        let base = Var::List(vec![Var::Int(1), Var::Int(2), Var::Int(3), Var::Int(4)]);
        let instr = Var::List(vec![Var::Int(5), Var::Int(6), Var::Int(7)]);
        assert_eq!(base.rangeset(instr.clone(), 0, 2), Err(Error::E_RANGE));
        assert_eq!(base.rangeset(instr.clone(), 1, 5), Err(Error::E_RANGE));
        assert_eq!(base.rangeset(instr.clone(), 2, 7), Err(Error::E_RANGE));
        assert_eq!(base.rangeset(instr, 1, 100), Err(Error::E_RANGE));
    }

    #[test]
    fn test_range() -> Result<(), Error> {
        // test on integer list
        let int_list = Var::List(vec![1.into(), 2.into(), 3.into(), 4.into(), 5.into()]);
        assert_eq!(
            int_list.range(2, 4)?,
            Var::List(vec![2.into(), 3.into(), 4.into()])
        );

        // test on string
        let string = Var::Str("hello world".to_string());
        assert_eq!(string.range(2, 7)?, Var::Str("ello w".to_string()));

        // test on empty list
        let empty_list = Var::List(vec![]);
        assert_eq!(empty_list.range(1, 0), Err(Error::E_RANGE));

        // test on out of range
        let int_list = Var::List(vec![1.into(), 2.into(), 3.into()]);
        assert_eq!(int_list.range(2, 4), Err(Error::E_RANGE));

        // test on type mismatch
        let var_int = Var::Int(10);
        assert_eq!(var_int.range(1, 5), Err(Error::E_TYPE));

        Ok(())
    }
}
