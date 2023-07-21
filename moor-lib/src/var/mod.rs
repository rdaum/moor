#![allow(non_camel_case_types, non_snake_case)]

use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Div, Mul, Neg, Sub};

use bincode::{Decode, Encode};
use decorum::R64;
use int_enum::IntEnum;
use num_traits::identities::Zero;

use crate::compiler::labels::Label;
use crate::var::error::Error;
use crate::var::error::Error::{E_RANGE, E_TYPE};

pub mod error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct Objid(pub i64);

impl Display for Objid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.0))
    }
}

pub const SYSTEM_OBJECT: Objid = Objid(0);
pub const NOTHING: Objid = Objid(-1);
pub const AMBIGUOUS: Objid = Objid(-2);
pub const FAILED_MATCH: Objid = Objid(-3);

pub const VAR_NONE: Var = Var {
    value: Variant::None,
};
pub const VAR_CLEAR: Var = Var {
    value: Variant::Clear,
};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum)]
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

#[derive(Clone, Encode, Decode)]
pub struct Var {
    value: Variant,
}

#[derive(Clone, Encode, Decode)]
pub enum Variant {
    Clear,
    None,
    Str(String),
    Obj(Objid),
    Int(i64),
    Float(f64),
    Err(Error),
    List(Vec<Var>),
    // Special for exception handling
    _Catch(Label),
    _Finally(Label),
    _Label(Label),
}

pub fn v_bool(b: bool) -> Var {
    Var {
        value: Variant::Int(if b { 1 } else { 0 }),
    }
}
pub fn v_int(i: i64) -> Var {
    Var {
        value: Variant::Int(i),
    }
}
pub fn v_float(f: f64) -> Var {
    Var {
        value: Variant::Float(f),
    }
}
pub fn v_str(s: &str) -> Var {
    Var {
        value: Variant::Str(s.to_string()),
    }
}
pub fn v_string(s: String) -> Var {
    Var {
        value: Variant::Str(s),
    }
}
pub fn v_objid(o: Objid) -> Var {
    Var {
        value: Variant::Obj(o),
    }
}
pub fn v_obj(o: i64) -> Var {
    Var {
        value: Variant::Obj(Objid(o)),
    }
}
pub fn v_err(e: Error) -> Var {
    Var {
        value: Variant::Err(e),
    }
}
pub fn v_list(l: Vec<Var>) -> Var {
    Var {
        value: Variant::List(l),
    }
}
pub fn v_label(l: Label) -> Var {
    Var {
        value: Variant::_Label(l),
    }
}
pub fn v_catch(l: Label) -> Var {
    Var {
        value: Variant::_Catch(l),
    }
}
pub fn v_finally(l: Label) -> Var {
    Var {
        value: Variant::_Finally(l),
    }
}

impl Var {
    pub fn variant(&self) -> &Variant {
        // TODO: We can produce this however we want, instead of actually holding "value" in the
        // the struct.  For 64-bit primitive values, we could just hold the value directly, but for
        // lists and strings this can be composed out of a byte buffer...
        // In this way we can do zero-copy for strings and lists direct from the DB.
        &self.value
    }

    pub fn type_id(&self) -> VarType {
        match self.variant() {
            Variant::Clear => VarType::TYPE_CLEAR,
            Variant::None => VarType::TYPE_NONE,
            Variant::Str(_) => VarType::TYPE_STR,
            Variant::Obj(_) => VarType::TYPE_OBJ,
            Variant::Int(_) => VarType::TYPE_INT,
            Variant::Float(_) => VarType::TYPE_FLOAT,
            Variant::Err(_) => VarType::TYPE_ERR,
            Variant::List(_) => VarType::TYPE_LIST,
            Variant::_Catch(_) => VarType::TYPE_CATCH,
            Variant::_Finally(_) => VarType::TYPE_FINALLY,
            Variant::_Label(_) => VarType::TYPE_CATCH,
        }
    }

    pub fn to_literal(&self) -> String {
        match self.variant() {
            Variant::None => "None".to_string(),
            Variant::Int(i) => i.to_string(),
            Variant::Float(f) => f.to_string(),
            Variant::Str(s) => format!("\"{}\"", s),
            Variant::Obj(o) => format!("{}", o),
            Variant::List(l) => {
                let mut result = String::new();
                result.push('{');
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(&v.to_literal());
                }
                result.push('}');
                result
            }
            Variant::Err(e) => e.name().to_string(),
            _ => "".to_string(),
        }
    }
}

impl Display for Var {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_literal().as_str())
    }
}

impl Debug for Var {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_literal().as_str())
    }
}

impl PartialEq<Self> for Var {
    fn eq(&self, other: &Self) -> bool {
        match (self.variant(), other.variant()) {
            (Variant::Clear, Variant::Clear) => true,
            (Variant::None, Variant::None) => true,
            (Variant::Str(l), Variant::Str(r)) => l == r,
            (Variant::Obj(l), Variant::Obj(r)) => l == r,
            (Variant::Int(l), Variant::Int(r)) => l == r,
            (Variant::Float(l), Variant::Float(r)) => l == r,
            (Variant::Err(l), Variant::Err(r)) => l == r,
            (Variant::List(l), Variant::List(r)) => l == r,
            (Variant::Clear, _) => false,
            (Variant::None, _) => false,
            (Variant::Str(_), _) => false,
            (Variant::Obj(_), _) => false,
            (Variant::Int(_), _) => false,
            (Variant::Float(_), _) => false,
            (Variant::Err(_), _) => false,
            (Variant::List(_), _) => false,
            (Variant::_Catch(a), Variant::_Catch(b)) => a == b,
            (Variant::_Finally(a), Variant::_Finally(b)) => a == b,
            (Variant::_Label(a), Variant::_Label(b)) => a == b,
            (Variant::_Catch(_a), _) => false,
            (Variant::_Label(_a), _) => false,
            (Variant::_Finally(_a), _) => false,
        }
    }
}

impl PartialOrd<Self> for Var {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.variant(), other.variant()) {
            (Variant::Clear, Variant::Clear) => Some(Ordering::Equal),
            (Variant::None, Variant::None) => Some(Ordering::Equal),
            (Variant::Str(l), Variant::Str(r)) => l.partial_cmp(r),
            (Variant::Obj(l), Variant::Obj(r)) => l.partial_cmp(r),
            (Variant::Int(l), Variant::Int(r)) => l.partial_cmp(r),
            (Variant::Float(l), Variant::Float(r)) => R64::from(*l).partial_cmp(&R64::from(*r)),
            (Variant::Err(l), Variant::Err(r)) => l.partial_cmp(r),
            (Variant::List(l), Variant::List(r)) => l.partial_cmp(r),
            (Variant::Clear, _) => Some(Ordering::Less),
            (Variant::None, _) => Some(Ordering::Less),
            (Variant::Str(_), _) => Some(Ordering::Less),
            (Variant::Obj(_), _) => Some(Ordering::Less),
            (Variant::Int(_), _) => Some(Ordering::Less),
            (Variant::Float(_), _) => Some(Ordering::Less),
            (Variant::Err(_), _) => Some(Ordering::Less),
            (Variant::List(_), _) => Some(Ordering::Less),
            (Variant::_Catch(a), Variant::_Catch(b)) => a.partial_cmp(b),
            (Variant::_Finally(a), Variant::_Finally(b)) => a.partial_cmp(b),
            (Variant::_Label(a), Variant::_Label(b)) => a.partial_cmp(b),
            (Variant::_Catch(_a), _) => Some(Ordering::Less),
            (Variant::_Label(_a), _) => Some(Ordering::Less),
            (Variant::_Finally(_a), _) => Some(Ordering::Less),
        }
    }
}

impl Ord for Var {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.variant(), other.variant()) {
            (Variant::Clear, Variant::Clear) => Ordering::Equal,
            (Variant::None, Variant::None) => Ordering::Equal,
            (Variant::Str(l), Variant::Str(r)) => l.cmp(r),
            (Variant::Obj(l), Variant::Obj(r)) => l.cmp(r),
            (Variant::Int(l), Variant::Int(r)) => l.cmp(r),
            (Variant::Float(l), Variant::Float(r)) => R64::from(*l).cmp(&R64::from(*r)),
            (Variant::Err(l), Variant::Err(r)) => l.cmp(r),
            (Variant::List(l), Variant::List(r)) => l.cmp(r),
            (Variant::Clear, _) => Ordering::Less,
            (Variant::None, _) => Ordering::Less,
            (Variant::Str(_), _) => Ordering::Less,
            (Variant::Obj(_), _) => Ordering::Less,
            (Variant::Int(_), _) => Ordering::Less,
            (Variant::Float(_), _) => Ordering::Less,
            (Variant::Err(_), _) => Ordering::Less,
            (Variant::List(_), _) => Ordering::Less,
            (Variant::_Catch(a), Variant::_Catch(b)) => a.cmp(b),
            (Variant::_Finally(a), Variant::_Finally(b)) => a.cmp(b),
            (Variant::_Label(a), Variant::_Label(b)) => a.cmp(b),
            (Variant::_Catch(_a), _) => Ordering::Less,
            (Variant::_Label(_a), _) => Ordering::Less,
            (Variant::_Finally(_a), _) => Ordering::Less,
        }
    }
}
impl Hash for Var {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let t = self.type_id() as u8;
        t.hash(state);
        match self.variant() {
            Variant::Clear => {}
            Variant::None => {}
            Variant::Str(s) => s.hash(state),
            Variant::Obj(o) => o.hash(state),
            Variant::Int(i) => i.hash(state),
            Variant::Float(f) => R64::from(*f).hash(state),
            Variant::Err(e) => e.hash(state),
            Variant::List(l) => l.hash(state),
            Variant::_Catch(l) => l.hash(state),
            Variant::_Finally(l) => l.hash(state),
            Variant::_Label(l) => l.hash(state),
        }
    }
}
impl Eq for Var {}

macro_rules! binary_numeric_coercion_op {
    ($op:tt ) => {
        pub fn $op(&self, v: &Var) -> Result<Var, Error> {
            match (self.variant(), v.variant()) {
                (Variant::Float(l), Variant::Float(r)) => Ok(v_float(l.$op(*r))),
                (Variant::Int(l), Variant::Int(r)) => Ok(v_int(l.$op(*r))),
                (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.$op(*r as f64))),
                (Variant::Int(l), Variant::Float(r)) => Ok(v_float((*l as f64).$op(*r))),
                (_, _) => Ok(v_err(E_TYPE)),
            }
        }
    };
}

impl Var {
    pub fn is_true(&self) -> bool {
        match self.variant() {
            Variant::Str(s) => !s.is_empty(),
            Variant::Int(i) => *i != 0,
            Variant::Float(f) => !f.is_zero(),
            Variant::List(l) => !l.is_empty(),
            _ => false,
        }
    }

    pub fn has_member(&self, v: &Var) -> Var {
        let Variant::List(l) = self.variant() else {
            return v_err(E_TYPE);
        };

        v_bool(l.contains(v))
    }

    binary_numeric_coercion_op!(mul);
    binary_numeric_coercion_op!(div);
    binary_numeric_coercion_op!(sub);

    pub fn add(&self, v: &Var) -> Result<Var, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(*l + *r)),
            (Variant::Int(l), Variant::Int(r)) => Ok(v_int(l + r)),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(*l + (*r as f64))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float(*l as f64 + *r)),
            (Variant::Str(s), Variant::Str(r)) => {
                let mut c = s.clone();
                c.push_str(r);
                Ok(v_str(c.as_str()))
            }
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn negative(&self) -> Result<Var, Error> {
        match self.variant() {
            Variant::Int(l) => Ok(v_int(-*l)),
            Variant::Float(f) => Ok(v_float(f.neg())),
            _ => Ok(v_err(E_TYPE)),
        }
    }

    pub fn modulus(&self, v: &Var) -> Result<Var, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(*l % *r)),
            (Variant::Int(l), Variant::Int(r)) => Ok(v_int(l % r)),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(*l % (*r as f64))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float(*l as f64 % (*r))),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn pow(&self, v: &Var) -> Result<Var, Error> {
        match (self.variant(), v.variant()) {
            (Variant::Float(l), Variant::Float(r)) => Ok(v_float(l.powf(*r))),
            (Variant::Int(l), Variant::Int(r)) => Ok(v_int(l.pow(*r as u32))),
            (Variant::Float(l), Variant::Int(r)) => Ok(v_float(l.powi(*r as i32))),
            (Variant::Int(l), Variant::Float(r)) => Ok(v_float((*l as f64).powf(*r))),
            (_, _) => Ok(v_err(E_TYPE)),
        }
    }

    pub fn index(&self, idx: usize) -> Result<Var, Error> {
        match self.variant() {
            Variant::List(l) => match l.get(idx) {
                None => Ok(v_err(E_RANGE)),
                Some(v) => Ok(v.clone()),
            },
            Variant::Str(s) => match s.get(idx..idx + 1) {
                None => Ok(v_err(E_RANGE)),
                Some(v) => Ok(v_str(v)),
            },
            _ => Ok(v_err(E_TYPE)),
        }
    }

    pub fn range(&self, from: i64, to: i64) -> Result<Var, Error> {
        match self.variant() {
            Variant::Str(s) => {
                let len = s.len() as i64;
                if from <= 0 || from > len + 1 || to < 1 || to > len {
                    return Ok(v_err(E_RANGE));
                }
                let (from, to) = (from as usize, to as usize);
                Ok(v_str(&s[from - 1..to]))
            }
            Variant::List(l) => {
                let len = l.len() as i64;
                if from <= 0 || from > len + 1 || to < 1 || to > len {
                    return Ok(v_err(E_RANGE));
                }
                let (from, to) = (from as usize, to as usize);
                let mut res = Vec::with_capacity(to - from + 1);
                for i in from..=to {
                    res.push(l[i - 1].clone());
                }
                Ok(v_list(res))
            }
            _ => Ok(v_err(E_TYPE)),
        }
    }

    pub fn rangeset(&self, value: Var, from: i64, to: i64) -> Result<Var, Error> {
        let (base_len, val_len) = match (self.variant(), value.variant()) {
            (Variant::Str(base_str), Variant::Str(val_str)) => {
                (base_str.len() as i64, val_str.len() as i64)
            }
            (Variant::List(base_list), Variant::List(val_list)) => {
                (base_list.len() as i64, val_list.len() as i64)
            }
            _ => return Ok(v_err(E_TYPE)),
        };

        if from <= 0 || from > base_len + 1 || to < 1 || to > base_len {
            return Ok(v_err(E_RANGE));
        }

        let lenleft = if from > 1 { from - 1 } else { 0 };
        let lenmiddle = val_len;
        let lenright = if base_len > to { base_len - to } else { 0 };
        let newsize = lenleft + lenmiddle + lenright;

        let (from, to) = (from as usize, to as usize);
        let ans = match (self.variant(), value.variant()) {
            (Variant::Str(base_str), Variant::Str(value_str)) => {
                let mut ans = String::with_capacity(newsize as usize);
                ans.push_str(&base_str[..from - 1]);
                ans.push_str(value_str);
                ans.push_str(&base_str[to..]);
                Variant::Str(ans)
            }
            (Variant::List(base_list), Variant::List(value_list)) => {
                let mut ans: Vec<Var> = Vec::with_capacity(newsize as usize);
                ans.extend_from_slice(&base_list[..from - 1]);
                ans.extend(value_list.iter().cloned());
                ans.extend_from_slice(&base_list[to..]);
                Variant::List(ans)
            }
            _ => unreachable!(),
        };

        Ok(Var { value: ans })
    }
}

impl<'a> From<&'a str> for Var {
    fn from(s: &'a str) -> Self {
        v_str(s)
    }
}

impl From<String> for Var {
    fn from(s: String) -> Self {
        v_str(&s)
    }
}

impl From<i64> for Var {
    fn from(i: i64) -> Self {
        v_int(i)
    }
}

impl From<f64> for Var {
    fn from(f: f64) -> Self {
        v_float(f)
    }
}

impl From<Objid> for Var {
    fn from(o: Objid) -> Self {
        v_objid(o)
    }
}

impl From<Vec<Var>> for Var {
    fn from(l: Vec<Var>) -> Self {
        v_list(l)
    }
}

impl From<Error> for Var {
    fn from(e: Error) -> Self {
        v_err(e)
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(v_int(1).add(&v_int(2)), Ok(v_int(3)));
        assert_eq!(v_int(1).add(&v_float(2.0)), Ok(v_float(3.0)));
        assert_eq!(v_float(1.).add(&v_int(2)), Ok(v_float(3.)));
        assert_eq!(v_float(1.).add(&v_float(2.)), Ok(v_float(3.)));
        assert_eq!(v_str("a").add(&v_str("b")), Ok(v_str("ab")));
    }

    #[test]
    fn test_sub() -> Result<(), Error> {
        assert_eq!(v_int(1).sub(&v_int(2))?, v_int(-1));
        assert_eq!(v_int(1).sub(&v_float(2.))?, v_float(-1.));
        assert_eq!(v_float(1.).sub(&v_int(2))?, v_float(-1.));
        assert_eq!(v_float(1.).sub(&v_float(2.))?, v_float(-1.));
        Ok(())
    }

    #[test]
    fn test_mul() -> Result<(), Error> {
        assert_eq!(v_int(1).mul(&v_int(2))?, v_int(2));
        assert_eq!(v_int(1).mul(&v_float(2.))?, v_float(2.));
        assert_eq!(v_float(1.).mul(&v_int(2))?, v_float(2.));
        assert_eq!(v_float(1.).mul(&v_float(2.))?, v_float(2.));
        Ok(())
    }

    #[test]
    fn test_div() -> Result<(), Error> {
        assert_eq!(v_int(1).div(&v_int(2))?, v_int(0));
        assert_eq!(v_int(1).div(&v_float(2.))?, v_float(0.5));
        assert_eq!(v_float(1.).div(&v_int(2))?, v_float(0.5));
        assert_eq!(v_float(1.).div(&v_float(2.))?, v_float(0.5));
        Ok(())
    }

    #[test]
    fn test_modulus() {
        assert_eq!(v_int(1).modulus(&v_int(2)), Ok(v_int(1)));
        assert_eq!(v_int(1).modulus(&v_float(2.)), Ok(v_float(1.)));
        assert_eq!(v_float(1.).modulus(&v_int(2)), Ok(v_float(1.)));
        assert_eq!(v_float(1.).modulus(&v_float(2.)), Ok(v_float(1.)));
        assert_eq!(v_str("moop").modulus(&v_int(2)), Ok(v_err(E_TYPE)));
    }

    #[test]
    fn test_pow() {
        assert_eq!(v_int(1).pow(&v_int(2)), Ok(v_int(1)));
        assert_eq!(v_int(2).pow(&v_int(2)), Ok(v_int(4)));
        assert_eq!(v_int(2).pow(&v_float(2.)), Ok(v_float(4.)));
        assert_eq!(v_float(2.).pow(&v_int(2)), Ok(v_float(4.)));
        assert_eq!(v_float(2.).pow(&v_float(2.)), Ok(v_float(4.)));
    }

    #[test]
    fn test_negative() {
        assert_eq!(v_int(1).negative(), Ok(v_int(-1)));
        assert_eq!(v_float(1.).negative(), Ok(v_float(-1.0)));
    }

    #[test]
    fn test_index() {
        assert_eq!(v_list(vec![v_int(1), v_int(2)]).index(0), Ok(v_int(1)));
        assert_eq!(v_list(vec![v_int(1), v_int(2)]).index(1), Ok(v_int(2)));
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).index(2),
            Ok(v_err(E_RANGE))
        );
        assert_eq!(v_str("ab").index(0), Ok(v_str("a")));
        assert_eq!(v_str("ab").index(1), Ok(v_str("b")));
        assert_eq!(v_str("ab").index(2), Ok(v_err(E_RANGE)));
    }

    #[test]
    fn test_eq() {
        assert_eq!(v_int(1), v_int(1));
        assert_eq!(v_float(1.), v_float(1.));
        assert_eq!(v_str("a"), v_str("a"));
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]),
            v_list(vec![v_int(1), v_int(2)])
        );
        assert_eq!(v_obj(1), v_obj(1));
        assert_eq!(v_err(E_TYPE), v_err(E_TYPE));
    }

    #[test]
    fn test_ne() {
        assert_ne!(v_int(1), v_int(2));
        assert_ne!(v_float(1.), v_float(2.));
        assert_ne!(v_str("a"), v_str("b"));
        assert_ne!(
            v_list(vec![v_int(1), v_int(2)]),
            v_list(vec![v_int(1), v_int(3)])
        );
        assert_ne!(v_obj(1), v_obj(2));
        assert_ne!(v_err(E_TYPE), v_err(E_RANGE));
    }

    #[test]
    fn test_lt() {
        assert!(v_int(1) < v_int(2));
        assert!(v_float(1.) < v_float(2.));
        assert!(v_str("a") < v_str("b"));
        assert!(v_list(vec![v_int(1), v_int(2)]) < v_list(vec![v_int(1), v_int(3)]));
        assert!(v_obj(1) < v_obj(2));
        assert!(v_err(E_TYPE) < v_err(E_RANGE));
    }

    #[test]
    fn test_le() {
        assert!(v_int(1) <= v_int(2));
        assert!(v_float(1.) <= v_float(2.));
        assert!(v_str("a") <= v_str("b"));
        assert!(v_list(vec![v_int(1), v_int(2)]) <= v_list(vec![v_int(1), v_int(3)]));
        assert!(v_obj(1) <= v_obj(2));
        assert!(v_err(E_TYPE) <= v_err(E_RANGE));
    }

    #[test]
    fn test_gt() {
        assert!(v_int(2) > v_int(1));
        assert!(v_float(2.) > v_float(1.));
        assert!(v_str("b") > v_str("a"));
        assert!(v_list(vec![v_int(1), v_int(3)]) > v_list(vec![v_int(1), v_int(2)]));
        assert!(v_obj(2) > v_obj(1));
        assert!(v_err(E_RANGE) > v_err(E_TYPE));
    }

    #[test]
    fn test_ge() {
        assert!(v_int(2) >= v_int(1));
        assert!(v_float(2.) >= v_float(1.));
        assert!(v_str("b") >= v_str("a"));
        assert!(v_list(vec![v_int(1), v_int(3)]) >= v_list(vec![v_int(1), v_int(2)]));
        assert!(v_obj(2) >= v_obj(1));
        assert!(v_err(E_RANGE) >= v_err(E_TYPE));
    }

    #[test]
    fn test_partial_cmp() {
        assert_eq!(v_int(1).partial_cmp(&v_int(1)), Some(Ordering::Equal));
        assert_eq!(v_float(1.).partial_cmp(&v_float(1.)), Some(Ordering::Equal));
        assert_eq!(v_str("a").partial_cmp(&v_str("a")), Some(Ordering::Equal));
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).partial_cmp(&v_list(vec![v_int(1), v_int(2)])),
            Some(Ordering::Equal)
        );
        assert_eq!(v_obj(1).partial_cmp(&v_obj(1)), Some(Ordering::Equal));
        assert_eq!(
            v_err(E_TYPE).partial_cmp(&v_err(E_TYPE)),
            Some(Ordering::Equal)
        );

        assert_eq!(v_int(1).partial_cmp(&v_int(2)), Some(Ordering::Less));
        assert_eq!(v_float(1.).partial_cmp(&v_float(2.)), Some(Ordering::Less));
        assert_eq!(v_str("a").partial_cmp(&v_str("b")), Some(Ordering::Less));
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).partial_cmp(&v_list(vec![v_int(1), v_int(3)])),
            Some(Ordering::Less)
        );
        assert_eq!(v_obj(1).partial_cmp(&v_obj(2)), Some(Ordering::Less));
        assert_eq!(
            v_err(E_TYPE).partial_cmp(&v_err(E_RANGE)),
            Some(Ordering::Less)
        );

        assert_eq!(v_int(2).partial_cmp(&v_int(1)), Some(Ordering::Greater));
        assert_eq!(
            v_float(2.).partial_cmp(&v_float(1.)),
            Some(Ordering::Greater)
        );
        assert_eq!(v_str("b").partial_cmp(&v_str("a")), Some(Ordering::Greater));
        assert_eq!(
            v_list(vec![v_int(1), v_int(3)]).partial_cmp(&v_list(vec![v_int(1), v_int(2)])),
            Some(Ordering::Greater)
        );
        assert_eq!(v_obj(2).partial_cmp(&v_obj(1)), Some(Ordering::Greater));
        assert_eq!(
            v_err(E_RANGE).partial_cmp(&v_err(E_TYPE)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn test_cmp() {
        assert_eq!(v_int(1).cmp(&v_int(1)), Ordering::Equal);
        assert_eq!(v_float(1.).cmp(&v_float(1.)), Ordering::Equal);
        assert_eq!(v_str("a").cmp(&v_str("a")), Ordering::Equal);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).cmp(&v_list(vec![v_int(1), v_int(2)])),
            Ordering::Equal
        );
        assert_eq!(v_obj(1).cmp(&v_obj(1)), Ordering::Equal);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_TYPE)), Ordering::Equal);

        assert_eq!(v_int(1).cmp(&v_int(2)), Ordering::Less);
        assert_eq!(v_float(1.).cmp(&v_float(2.)), Ordering::Less);
        assert_eq!(v_str("a").cmp(&v_str("b")), Ordering::Less);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).cmp(&v_list(vec![v_int(1), v_int(3)])),
            Ordering::Less
        );
        assert_eq!(v_obj(1).cmp(&v_obj(2)), Ordering::Less);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_RANGE)), Ordering::Less);

        assert_eq!(v_int(2).cmp(&v_int(1)), Ordering::Greater);
        assert_eq!(v_float(2.).cmp(&v_float(1.)), Ordering::Greater);
        assert_eq!(v_str("b").cmp(&v_str("a")), Ordering::Greater);
        assert_eq!(
            v_list(vec![v_int(1), v_int(3)]).cmp(&v_list(vec![v_int(1), v_int(2)])),
            Ordering::Greater
        );
        assert_eq!(v_obj(2).cmp(&v_obj(1)), Ordering::Greater);
        assert_eq!(v_err(E_RANGE).cmp(&v_err(E_TYPE)), Ordering::Greater);
    }

    #[test]
    fn test_partial_ord() {
        assert_eq!(v_int(1).partial_cmp(&v_int(1)).unwrap(), Ordering::Equal);
        assert_eq!(
            v_float(1.).partial_cmp(&v_float(1.)).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            v_str("a").partial_cmp(&v_str("a")).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)])
                .partial_cmp(&v_list(vec![v_int(1), v_int(2)]))
                .unwrap(),
            Ordering::Equal
        );
        assert_eq!(v_obj(1).partial_cmp(&v_obj(1)).unwrap(), Ordering::Equal);
        assert_eq!(
            v_err(E_TYPE).partial_cmp(&v_err(E_TYPE)).unwrap(),
            Ordering::Equal
        );

        assert_eq!(v_int(1).partial_cmp(&v_int(2)).unwrap(), Ordering::Less);
        assert_eq!(
            v_float(1.).partial_cmp(&v_float(2.)).unwrap(),
            Ordering::Less
        );
        assert_eq!(v_str("a").partial_cmp(&v_str("b")).unwrap(), Ordering::Less);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)])
                .partial_cmp(&v_list(vec![v_int(1), v_int(3)]))
                .unwrap(),
            Ordering::Less
        );
        assert_eq!(v_obj(1).partial_cmp(&v_obj(2)).unwrap(), Ordering::Less);
        assert_eq!(
            v_err(E_TYPE).partial_cmp(&v_err(E_RANGE)).unwrap(),
            Ordering::Less
        );

        assert_eq!(v_int(2).partial_cmp(&v_int(1)).unwrap(), Ordering::Greater);
        assert_eq!(
            v_float(2.).partial_cmp(&v_float(1.)).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            v_str("b").partial_cmp(&v_str("a")).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            v_list(vec![v_int(1), v_int(3)])
                .partial_cmp(&v_list(vec![v_int(1), v_int(2)]))
                .unwrap(),
            Ordering::Greater
        );
        assert_eq!(v_obj(2).partial_cmp(&v_obj(1)).unwrap(), Ordering::Greater);
        assert_eq!(
            v_err(E_RANGE).partial_cmp(&v_err(E_TYPE)).unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn test_ord() {
        assert_eq!(v_int(1).cmp(&v_int(1)), Ordering::Equal);
        assert_eq!(v_float(1.).cmp(&v_float(1.)), Ordering::Equal);
        assert_eq!(v_str("a").cmp(&v_str("a")), Ordering::Equal);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).cmp(&v_list(vec![v_int(1), v_int(2)])),
            Ordering::Equal
        );
        assert_eq!(v_obj(1).cmp(&v_obj(1)), Ordering::Equal);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_TYPE)), Ordering::Equal);

        assert_eq!(v_int(1).cmp(&v_int(2)), Ordering::Less);
        assert_eq!(v_float(1.).cmp(&v_float(2.)), Ordering::Less);
        assert_eq!(v_str("a").cmp(&v_str("b")), Ordering::Less);
        assert_eq!(
            v_list(vec![v_int(1), v_int(2)]).cmp(&v_list(vec![v_int(1), v_int(3)])),
            Ordering::Less
        );
        assert_eq!(v_obj(1).cmp(&v_obj(2)), Ordering::Less);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_RANGE)), Ordering::Less);

        assert_eq!(v_int(2).cmp(&v_int(1)), Ordering::Greater);
        assert_eq!(v_float(2.).cmp(&v_float(1.)), Ordering::Greater);
        assert_eq!(v_str("b").cmp(&v_str("a")), Ordering::Greater);
        assert_eq!(
            v_list(vec![v_int(1), v_int(3)]).cmp(&v_list(vec![v_int(1), v_int(2)])),
            Ordering::Greater
        );
        assert_eq!(v_obj(2).cmp(&v_obj(1)), Ordering::Greater);
        assert_eq!(v_err(E_RANGE).cmp(&v_err(E_TYPE)), Ordering::Greater);
    }

    #[test]
    fn test_is_true() {
        assert!(v_int(1).is_true());
        assert!(v_float(1.).is_true());
        assert!(v_str("a").is_true());
        assert!(v_list(vec![v_int(1), v_int(2)]).is_true());
        assert!(!v_obj(1).is_true());
        assert!(!v_err(E_TYPE).is_true());
    }

    #[test]
    fn test_listrangeset() {
        let base = v_list(vec![v_int(1), v_int(2), v_int(3), v_int(4)]);

        // {1,2,3,4}[1..2] = {"a", "b", "c"} => {1, "a", "b", "c", 4}
        let value = v_list(vec![v_str("a"), v_str("b"), v_str("c")]);
        let expected = v_list(vec![v_int(1), v_str("a"), v_str("b"), v_str("c"), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {"a"} => {1, "a", 4}
        let value = v_list(vec![v_str("a")]);
        let expected = v_list(vec![v_int(1), v_str("a"), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {} => {1,4}
        let value = v_list(vec![]);
        let expected = v_list(vec![v_int(1), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {"a", "b"} => {1, "a", "b", 4}
        let value = v_list(vec![v_str("a"), v_str("b")]);
        let expected = v_list(vec![v_int(1), v_str("a"), v_str("b"), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);
    }

    #[test]
    fn test_strrangeset() {
        // Test interior insertion
        let base = v_str("12345");
        let value = v_str("abc");
        let expected = v_str("1abc45");
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior replacement
        let base = v_str("12345");
        let value = v_str("ab");
        let expected = v_str("1ab45");
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior deletion
        let base = v_str("12345");
        let value = v_str("");
        let expected = v_str("145");
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));

        // Test interior subtraction
        let base = v_str("12345");
        let value = v_str("z");
        let expected = v_str("1z45");
        let result = base.rangeset(value, 2, 3);
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_rangeset_check_negative() {
        // Test negative cases for strings
        let base = v_str("abcdef");
        let instr = v_str("ghi");
        assert_eq!(base.rangeset(instr.clone(), 1, 0), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr.clone(), 0, 3), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr.clone(), 2, 7), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr, 1, 100), Ok(v_err(E_RANGE)));

        // Test negative cases for lists
        let base = v_list(vec![v_int(1), v_int(2), v_int(3), v_int(4)]);
        let instr = v_list(vec![v_int(5), v_int(6), v_int(7)]);
        assert_eq!(base.rangeset(instr.clone(), 0, 2), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr.clone(), 1, 5), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr.clone(), 2, 7), Ok(v_err(E_RANGE)));
        assert_eq!(base.rangeset(instr, 1, 100), Ok(v_err(E_RANGE)));
    }

    #[test]
    fn test_range() -> Result<(), Error> {
        // test on integer list
        let int_list = v_list(vec![1.into(), 2.into(), 3.into(), 4.into(), 5.into()]);
        assert_eq!(
            int_list.range(2, 4)?,
            v_list(vec![2.into(), 3.into(), 4.into()])
        );

        // test on string
        let string = v_str("hello world");
        assert_eq!(string.range(2, 7)?, v_str("ello w"));

        // test on empty list
        let empty_list = v_list(vec![]);
        assert_eq!(empty_list.range(1, 0), Ok(v_err(E_RANGE)));
        // test on out of range
        let int_list = v_list(vec![1.into(), 2.into(), 3.into()]);
        assert_eq!(int_list.range(2, 4), Ok(v_err(E_RANGE)));
        // test on type mismatch
        let var_int = v_int(10);
        assert_eq!(var_int.range(1, 5), Ok(v_err(E_TYPE)));

        Ok(())
    }
}
