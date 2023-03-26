use std::ops::{Add, Div, Mul, Neg, Sub};

use decorum::{Real, R64};
use int_enum::IntEnum;
use num_traits::identities::Zero;
use serde_derive::{Deserialize, Serialize};

use crate::model::var::Error::{E_RANGE, E_TYPE};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct Objid(pub i64);

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, IntEnum, Ord, PartialOrd, Serialize, Deserialize)]
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
                (Var::Float(l), Var::Float(r)) => Var::Float(l.$op(*r)),
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
            Var::Str(s) => !s.is_empty(),
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

    pub fn add(&self, v: &Var) -> Var {
        match (self, v) {
            (Var::Float(l), Var::Float(r)) => Var::Float(*l + *r),
            (Var::Int(l), Var::Int(r)) => Var::Int(l + r),
            (Var::Float(l), Var::Int(r)) => Var::Float(*l + (*r as f64)),
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

    pub fn modulus(&self, v: &Var) -> Var {
        match (self, v) {
            (Var::Float(l), Var::Float(r)) => Var::Float(*l % *r),
            (Var::Int(l), Var::Int(r)) => Var::Int(l % r),
            (Var::Float(l), Var::Int(r)) => Var::Float(*l % (*r as f64)),
            (Var::Int(l), Var::Float(r)) => {
                let l = R64::from(*l as f64);
                Var::Float(l % (*r))
            }
            (_, _) => Var::Err(E_TYPE),
        }
    }

    // TODO this likely does not match MOO's impl, which is ... custom. but may not matter.
    pub fn pow(&self, v: &Var) -> Var {
        match (self, v) {
            (Var::Float(l), Var::Float(r)) => Var::Float(l.powf(*r)),
            (Var::Int(l), Var::Int(r)) => Var::Int(l.pow(*r as u32)),
            (Var::Float(l), Var::Int(r)) => Var::Float(l.powi(*r as i32)),
            (Var::Int(l), Var::Float(r)) => {
                let l = R64::from(*l as f64);
                Var::Float(l.powf(*r))
            }
            (_, _) => Var::Err(E_TYPE),
        }
    }

    pub fn negative(&self) -> Var {
        match self {
            Var::Int(l) => Var::Int(-*l),
            Var::Float(f) => Var::Float(f.neg()),
            _ => Var::Err(E_TYPE),
        }
    }

    pub fn index(&self, idx: usize) -> Var {
        match self {
            Var::List(l) => match l.get(idx) {
                None => Var::Err(E_RANGE),
                Some(v) => v.clone(),
            },
            Var::Str(s) => match s.get(idx..idx + 1) {
                None => Var::Err(E_RANGE),
                Some(v) => Var::Str(String::from(v)),
            },
            _ => Var::Err(E_TYPE),
        }
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
        Self::Float(R64::from(f))
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

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(Var::Int(1).add(&Var::Int(2)), Var::Int(3));
        assert_eq!(
            Var::Int(1).add(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(3.0))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).add(&Var::Int(2)),
            Var::Float(R64::from(3.0))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).add(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(3.0))
        );
        assert_eq!(
            Var::Str(String::from("a")).add(&Var::Str(String::from("b"))),
            Var::Str(String::from("ab"))
        );
    }

    #[test]
    fn test_sub() {
        assert_eq!(Var::Int(1).sub(&Var::Int(2)), Var::Int(-1));
        assert_eq!(
            Var::Int(1).sub(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(-1.0))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).sub(&Var::Int(2)),
            Var::Float(R64::from(-1.0))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).sub(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(-1.0))
        );
    }

    #[test]
    fn test_mul() {
        assert_eq!(Var::Int(1).mul(&Var::Int(2)), Var::Int(2));
        assert_eq!(
            Var::Int(1).mul(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(2.0))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).mul(&Var::Int(2)),
            Var::Float(R64::from(2.0))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).mul(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(2.0))
        );
    }

    #[test]
    fn test_div() {
        assert_eq!(Var::Int(1).div(&Var::Int(2)), Var::Int(0));
        assert_eq!(
            Var::Int(1).div(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(0.5))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).div(&Var::Int(2)),
            Var::Float(R64::from(0.5))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).div(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(0.5))
        );
    }

    #[test]
    fn test_modulus() {
        assert_eq!(Var::Int(1).modulus(&Var::Int(2)), Var::Int(1));
        assert_eq!(
            Var::Int(1).modulus(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(1.0))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).modulus(&Var::Int(2)),
            Var::Float(R64::from(1.0))
        );
        assert_eq!(
            Var::Float(R64::from(1.0)).modulus(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(1.0))
        );
    }

    #[test]
    fn test_pow() {
        assert_eq!(Var::Int(1).pow(&Var::Int(2)), Var::Int(1));
        assert_eq!(Var::Int(2).pow(&Var::Int(2)), Var::Int(4));
        assert_eq!(
            Var::Int(2).pow(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(4.0))
        );
        assert_eq!(
            Var::Float(R64::from(2.0)).pow(&Var::Int(2)),
            Var::Float(R64::from(4.0))
        );
        assert_eq!(
            Var::Float(R64::from(2.0)).pow(&Var::Float(R64::from(2.0))),
            Var::Float(R64::from(4.0))
        );
    }

    #[test]
    fn test_negative() {
        assert_eq!(Var::Int(1).negative(), Var::Int(-1));
        assert_eq!(
            Var::Float(R64::from(1.0)).negative(),
            Var::Float(R64::from(-1.0))
        );
    }

    #[test]
    fn test_index() {
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)]).index(0),
            Var::Int(1)
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)]).index(1),
            Var::Int(2)
        );
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)]).index(2),
            Var::Err(E_RANGE)
        );
        assert_eq!(
            Var::Str(String::from("ab")).index(0),
            Var::Str(String::from("a"))
        );
        assert_eq!(
            Var::Str(String::from("ab")).index(1),
            Var::Str(String::from("b"))
        );
        assert_eq!(Var::Str(String::from("ab")).index(2), Var::Err(E_RANGE));
    }

    #[test]
    fn test_eq() {
        assert_eq!(Var::Int(1), Var::Int(1));
        assert_eq!(Var::Float(R64::from(1.0)), Var::Float(R64::from(1.0)));
        assert_eq!(Var::Str(String::from("a")), Var::Str(String::from("a")));
        assert_eq!(
            Var::List(vec![Var::Int(1), Var::Int(2)]),
            Var::List(vec![Var::Int(1), Var::Int(2)])
        );
        assert_eq!(Var::Obj(Objid(1)), Var::Obj(Objid(1)));
        assert_eq!(Var::Err(E_TYPE), Var::Err(E_TYPE));
    }

    #[test]
    fn test_ne() {
        assert_ne!(Var::Int(1), Var::Int(2));
        assert_ne!(Var::Float(R64::from(1.0)), Var::Float(R64::from(2.0)));
        assert_ne!(Var::Str(String::from("a")), Var::Str(String::from("b")));
        assert_ne!(
            Var::List(vec![Var::Int(1), Var::Int(2)]),
            Var::List(vec![Var::Int(1), Var::Int(3)])
        );
        assert_ne!(Var::Obj(Objid(1)), Var::Obj(Objid(2)));
        assert_ne!(Var::Err(E_TYPE), Var::Err(E_RANGE));
    }

    #[test]
    fn test_lt() {
        assert!(Var::Int(1) < Var::Int(2));
        assert!(Var::Float(R64::from(1.0)) < Var::Float(R64::from(2.0)));
        assert!(Var::Str(String::from("a")) < Var::Str(String::from("b")));
        assert!(
            Var::List(vec![Var::Int(1), Var::Int(2)]) < Var::List(vec![Var::Int(1), Var::Int(3)])
        );
        assert!(Var::Obj(Objid(1)) < Var::Obj(Objid(2)));
        assert!(Var::Err(E_TYPE) < Var::Err(E_RANGE));
    }

    #[test]
    fn test_le() {
        assert!(Var::Int(1) <= Var::Int(2));
        assert!(Var::Float(R64::from(1.0)) <= Var::Float(R64::from(2.0)));
        assert!(Var::Str(String::from("a")) <= Var::Str(String::from("b")));
        assert!(
            Var::List(vec![Var::Int(1), Var::Int(2)]) <= Var::List(vec![Var::Int(1), Var::Int(3)])
        );
        assert!(Var::Obj(Objid(1)) <= Var::Obj(Objid(2)));
        assert!(Var::Err(E_TYPE) <= Var::Err(E_RANGE));
    }

    #[test]
    fn test_gt() {
        assert!(Var::Int(2) > Var::Int(1));
        assert!(Var::Float(R64::from(2.0)) > Var::Float(R64::from(1.0)));
        assert!(Var::Str(String::from("b")) > Var::Str(String::from("a")));
        assert!(
            Var::List(vec![Var::Int(1), Var::Int(3)]) > Var::List(vec![Var::Int(1), Var::Int(2)])
        );
        assert!(Var::Obj(Objid(2)) > Var::Obj(Objid(1)));
        assert!(Var::Err(E_RANGE) > Var::Err(E_TYPE));
    }

    #[test]
    fn test_ge() {
        assert!(Var::Int(2) >= Var::Int(1));
        assert!(Var::Float(R64::from(2.0)) >= Var::Float(R64::from(1.0)));
        assert!(Var::Str(String::from("b")) >= Var::Str(String::from("a")));
        assert!(
            Var::List(vec![Var::Int(1), Var::Int(3)]) >= Var::List(vec![Var::Int(1), Var::Int(2)])
        );
        assert!(Var::Obj(Objid(2)) >= Var::Obj(Objid(1)));
        assert!(Var::Err(E_RANGE) >= Var::Err(E_TYPE));
    }

    #[test]
    fn test_partial_cmp() {
        assert_eq!(Var::Int(1).partial_cmp(&Var::Int(1)), Some(Ordering::Equal));
        assert_eq!(
            Var::Float(R64::from(1.0)).partial_cmp(&Var::Float(R64::from(1.0))),
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
            Var::Err(E_TYPE).partial_cmp(&Var::Err(E_TYPE)),
            Some(Ordering::Equal)
        );

        assert_eq!(Var::Int(1).partial_cmp(&Var::Int(2)), Some(Ordering::Less));
        assert_eq!(
            Var::Float(R64::from(1.0)).partial_cmp(&Var::Float(R64::from(2.0))),
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
            Var::Err(E_TYPE).partial_cmp(&Var::Err(E_RANGE)),
            Some(Ordering::Less)
        );

        assert_eq!(
            Var::Int(2).partial_cmp(&Var::Int(1)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Var::Float(R64::from(2.0)).partial_cmp(&Var::Float(R64::from(1.0))),
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
            Var::Err(E_RANGE).partial_cmp(&Var::Err(E_TYPE)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn test_cmp() {
        assert_eq!(Var::Int(1).cmp(&Var::Int(1)), Ordering::Equal);
        assert_eq!(
            Var::Float(R64::from(1.0)).cmp(&Var::Float(R64::from(1.0))),
            Ordering::Equal
        );
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
        assert_eq!(Var::Err(E_TYPE).cmp(&Var::Err(E_TYPE)), Ordering::Equal);

        assert_eq!(Var::Int(1).cmp(&Var::Int(2)), Ordering::Less);
        assert_eq!(
            Var::Float(R64::from(1.0)).cmp(&Var::Float(R64::from(2.0))),
            Ordering::Less
        );
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
        assert_eq!(Var::Err(E_TYPE).cmp(&Var::Err(E_RANGE)), Ordering::Less);

        assert_eq!(Var::Int(2).cmp(&Var::Int(1)), Ordering::Greater);
        assert_eq!(
            Var::Float(R64::from(2.0)).cmp(&Var::Float(R64::from(1.0))),
            Ordering::Greater
        );
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
        assert_eq!(Var::Err(E_RANGE).cmp(&Var::Err(E_TYPE)), Ordering::Greater);
    }

    #[test]
    fn test_partial_ord() {
        assert_eq!(
            Var::Int(1).partial_cmp(&Var::Int(1)).unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            Var::Float(R64::from(1.0))
                .partial_cmp(&Var::Float(R64::from(1.0)))
                .unwrap(),
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
            Var::Err(E_TYPE).partial_cmp(&Var::Err(E_TYPE)).unwrap(),
            Ordering::Equal
        );

        assert_eq!(
            Var::Int(1).partial_cmp(&Var::Int(2)).unwrap(),
            Ordering::Less
        );
        assert_eq!(
            Var::Float(R64::from(1.0))
                .partial_cmp(&Var::Float(R64::from(2.0)))
                .unwrap(),
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
            Var::Err(E_TYPE).partial_cmp(&Var::Err(E_RANGE)).unwrap(),
            Ordering::Less
        );

        assert_eq!(
            Var::Int(2).partial_cmp(&Var::Int(1)).unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            Var::Float(R64::from(2.0))
                .partial_cmp(&Var::Float(R64::from(1.0)))
                .unwrap(),
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
            Var::Err(E_RANGE).partial_cmp(&Var::Err(E_TYPE)).unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn test_ord() {
        assert_eq!(Var::Int(1).cmp(&Var::Int(1)), Ordering::Equal);
        assert_eq!(
            Var::Float(R64::from(1.0)).cmp(&Var::Float(R64::from(1.0))),
            Ordering::Equal
        );
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
        assert_eq!(Var::Err(E_TYPE).cmp(&Var::Err(E_TYPE)), Ordering::Equal);

        assert_eq!(Var::Int(1).cmp(&Var::Int(2)), Ordering::Less);
        assert_eq!(
            Var::Float(R64::from(1.0)).cmp(&Var::Float(R64::from(2.0))),
            Ordering::Less
        );
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
        assert_eq!(Var::Err(E_TYPE).cmp(&Var::Err(E_RANGE)), Ordering::Less);

        assert_eq!(Var::Int(2).cmp(&Var::Int(1)), Ordering::Greater);
        assert_eq!(
            Var::Float(R64::from(2.0)).cmp(&Var::Float(R64::from(1.0))),
            Ordering::Greater
        );
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
        assert_eq!(Var::Err(E_RANGE).cmp(&Var::Err(E_TYPE)), Ordering::Greater);
    }

    #[test]
    fn test_is_true() {
        assert!(Var::Int(1).is_true());
        assert!(Var::Float(R64::from(1.0)).is_true());
        assert!(Var::Str(String::from("a")).is_true());
        assert!(Var::List(vec![Var::Int(1), Var::Int(2)]).is_true());
        assert!(!Var::Obj(Objid(1)).is_true());
        assert!(!Var::Err(E_TYPE).is_true());
    }
}
