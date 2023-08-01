use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::{Div, Mul, Neg, Sub};


use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use decorum::R64;
use lazy_static::lazy_static;
use num_traits::Zero;

use crate::values::error::Error;
use crate::values::error::Error::{E_RANGE, E_TYPE};
use crate::values::list::List;
use crate::values::objid::Objid;
use crate::values::string::Str;
use crate::values::variant::Variant;
use crate::values::VarType;

lazy_static! {
    static ref VAR_NONE: Var = Var::new(Variant::None);
    static ref VAR_CLEAR: Var = Var::new(Variant::Clear);
    static ref VAR_EMPTY_LIST: Var = Var::new(Variant::List(List::new()));
    static ref VAR_EMPTY_STR: Var = Var::new(Variant::Str(Str::from_str("")));
}

#[derive(Clone)]
pub struct Var {
    value: Variant,
}

impl Var {
    pub fn new(value: Variant) -> Self {
        Self { value }
    }
}

impl Encode for Var {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        let inner = self.variant();
        inner.encode(encoder)
    }
}

impl Decode for Var {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let inner = Variant::decode(decoder)?;
        Ok(Var::new(inner))
    }
}

impl<'de> BorrowDecode<'de> for Var {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let inner = Variant::borrow_decode(decoder)?;
        Ok(Var::new(inner))
    }
}

pub fn v_bool(b: bool) -> Var {
    Var::new(Variant::Int(if b { 1 } else { 0 }))
}

pub fn v_int(i: i64) -> Var {
    Var::new(Variant::Int(i))
}

pub fn v_float(f: f64) -> Var {
    Var::new(Variant::Float(f))
}

pub fn v_str(s: &str) -> Var {
    Var::new(Variant::Str(Str::from_str(s)))
}

pub fn v_string(s: String) -> Var {
    Var::new(Variant::Str(Str::from_string(s)))
}

pub fn v_objid(o: Objid) -> Var {
    Var::new(Variant::Obj(o))
}

pub fn v_obj(o: i64) -> Var {
    Var::new(Variant::Obj(Objid(o)))
}

pub fn v_err(e: Error) -> Var {
    Var::new(Variant::Err(e))
}

pub fn v_list(l: Vec<Var>) -> Var {
    Var::new(Variant::List(List::from_vec(l)))
}

// Macro to call v_list with vector arguments to construct instead of having to do v_list(vec![...])
macro_rules! v_lst {
    () => (
        $crate::values::var::v_empty_list()
    );
    ($($x:expr),+ $(,)?) => (
        vec![$($x),+]
    );
}

pub fn v_empty_list() -> Var {
    VAR_EMPTY_LIST.clone()
}

pub fn v_empty_str() -> Var {
    VAR_EMPTY_STR.clone()
}
pub fn v_clear() -> Var {
    VAR_CLEAR.clone()
}

pub fn v_none() -> Var {
    VAR_NONE.clone()
}

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
    pub fn variant(&self) -> &Variant {
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
        }
    }
}

impl Eq for Var {}

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

    /// 1-indexed position of the first occurrence of `v` in `self`, or `E_TYPE` if `self` is not a
    /// list.
    pub fn index_in(&self, v: &Var) -> Var {
        let Variant::List(l) = self.variant() else {
            return v_err(E_TYPE);
        };

        match l.iter().position(|x| x == v) {
            None => v_int(0),
            Some(i) => v_int(i as i64 + 1),
        }
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
            (Variant::Str(s), Variant::Str(r)) => Ok(s.append(r)),
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
            Variant::Str(s) => match s.get(idx) {
                None => Ok(v_err(E_RANGE)),
                Some(v) => Ok(v.clone()),
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
                Ok(s.get_range(from - 1..to).unwrap())
            }
            Variant::List(l) => {
                let len = l.len() as i64;
                if to < from {
                    return Ok(v_empty_list());
                }
                if from <= 0 || from > len + 1 || to < 1 || to > len {
                    return Ok(v_err(E_RANGE));
                }
                let mut res = Vec::with_capacity((to - from + 1) as usize);
                for i in from..=to {
                    res.push(l[(i - 1) as usize].clone());
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
            (Variant::Str(base_str), Variant::Str(_value_str)) => {
                let ans = base_str.get_range(0..from - 1).unwrap_or_else(v_empty_str);
                let ans = ans.add(&value)?;
                
                ans.add(
                    &base_str
                        .get_range(to..base_str.len())
                        .unwrap_or_else(v_empty_str),
                )?
            }
            (Variant::List(base_list), Variant::List(value_list)) => {
                let mut ans: Vec<Var> = Vec::with_capacity(newsize as usize);
                ans.extend_from_slice(&base_list[..from - 1]);
                ans.extend(value_list.iter().cloned());
                ans.extend_from_slice(&base_list[to..]);
                v_list(ans)
            }
            _ => unreachable!(),
        };

        Ok(ans)
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
