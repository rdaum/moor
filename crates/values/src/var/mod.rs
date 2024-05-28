// Copyright (C) 2024 Ryan Daum <ryan.daum@gmail.com>
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, version 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

#![allow(non_camel_case_types, non_snake_case)]

use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use decorum::R64;
use lazy_static::lazy_static;
use strum::FromRepr;

use crate::util::quote_str;

pub use crate::var::error::{Error, ErrorPack};
pub use crate::var::list::List;
pub use crate::var::objid::Objid;
pub use crate::var::string::Str;
pub use crate::var::variant::Variant;

mod error;
mod list;
mod objid;
mod string;
mod variant;
mod varops;

lazy_static! {
    static ref VAR_NONE: Var = Variant::None.into();
    static ref VAR_EMPTY_LIST: Var = Variant::List(List::new()).into();
    static ref VAR_EMPTY_STR: Var = Var::new(Variant::Str(Str::from_str("").unwrap()));
}

// Macro to call v_list with vector arguments to construct instead of having to do v_list(&[...])
#[allow(unused_macros)]
macro_rules! v_lst {
    () => (
        $crate::values::var::v_empty_list()
    );
    ($($x:expr),+ $(,)?) => (
        vec![$($x),+]
    );
}

/// Integer encoding of values as represented in a `LambdaMOO` textdump, and by `bf_typeof`
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
pub enum VarType {
    TYPE_INT = 0,
    TYPE_OBJ = 1,
    TYPE_STR = 2,
    TYPE_ERR = 3,
    TYPE_LIST = 4,
    TYPE_NONE = 6,  // in uninitialized MOO variables */
    TYPE_LABEL = 7, // present only in textdump */
    TYPE_FLOAT = 9,
}

/// Var is our variant type / tagged union used to represent MOO's dynamically typed values.
#[derive(Clone)]
pub struct Var {
    value: Variant,
}

impl Var {
    #[must_use]
    pub(crate) fn new(value: Variant) -> Self {
        Self { value }
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        match self.variant() {
            Variant::Obj(o) => o.is_sysobj(),
            _ => false,
        }
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
        Ok(Self::new(inner))
    }
}

impl<'de> BorrowDecode<'de> for Var {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let inner = Variant::borrow_decode(decoder)?;
        Ok(Self::new(inner))
    }
}

#[must_use]
pub fn v_bool(b: bool) -> Var {
    Var::new(Variant::Int(i64::from(b)))
}

#[must_use]
pub fn v_int(i: i64) -> Var {
    Var::new(Variant::Int(i))
}

#[must_use]
pub fn v_float(f: f64) -> Var {
    Var::new(Variant::Float(f))
}

#[must_use]
pub fn v_str(s: &str) -> Var {
    Var::new(Variant::Str(Str::from_str(s).unwrap()))
}

#[must_use]
pub fn v_string(s: String) -> Var {
    Var::new(Variant::Str(Str::from_string(s)))
}

#[must_use]
pub fn v_objid(o: Objid) -> Var {
    Var::new(Variant::Obj(o))
}

#[must_use]
pub fn v_obj(o: i64) -> Var {
    Var::new(Variant::Obj(Objid(o)))
}

#[must_use]
pub fn v_err(e: Error) -> Var {
    Var::new(Variant::Err(e))
}

#[must_use]
pub fn v_list(l: &[Var]) -> Var {
    Var::new(Variant::List(List::from_vec(l.to_vec())))
}

#[must_use]
pub fn v_listv(l: Vec<Var>) -> Var {
    Var::new(Variant::List(List::from_vec(l)))
}

#[must_use]
pub fn v_empty_list() -> Var {
    VAR_EMPTY_LIST.clone()
}

#[must_use]
pub fn v_empty_str() -> Var {
    VAR_EMPTY_STR.clone()
}

#[must_use]
pub fn v_none() -> Var {
    VAR_NONE.clone()
}

impl Var {
    /// Return a reference to the inner variant that this Var is wrapping.
    #[must_use]
    #[inline]
    pub fn variant(&self) -> &Variant {
        &self.value
    }

    #[must_use]
    #[inline]
    pub fn variant_mut(&mut self) -> &mut Variant {
        &mut self.value
    }

    /// Destroy the Var and return the inner variant that it was wrapping.
    #[must_use]
    #[inline]
    pub fn take_variant(self) -> Variant {
        self.value
    }

    #[must_use]
    pub fn type_id(&self) -> VarType {
        match self.variant() {
            Variant::None => VarType::TYPE_NONE,
            Variant::Str(_) => VarType::TYPE_STR,
            Variant::Obj(_) => VarType::TYPE_OBJ,
            Variant::Int(_) => VarType::TYPE_INT,
            Variant::Float(_) => VarType::TYPE_FLOAT,
            Variant::Err(_) => VarType::TYPE_ERR,
            Variant::List(_) => VarType::TYPE_LIST,
        }
    }

    #[must_use]
    pub fn eq_case_sensitive(&self, other: &Self) -> bool {
        match (self.variant(), other.variant()) {
            (Variant::Str(l), Variant::Str(r)) => l.as_str().eq(r.as_str()),
            _ => self == other,
        }
    }

    #[must_use]
    pub fn to_literal(&self) -> String {
        match self.variant() {
            Variant::None => "None".to_string(),
            Variant::Int(i) => i.to_string(),
            Variant::Float(f) => format!("{f:?}"),
            Variant::Str(s) => quote_str(s.as_str()),
            Variant::Obj(o) => format!("{o}"),
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
            (Variant::None, Variant::None) => true,
            (Variant::Str(l), Variant::Str(r)) => l == r,
            (Variant::Obj(l), Variant::Obj(r)) => l == r,
            (Variant::Int(l), Variant::Int(r)) => l == r,
            (Variant::Float(l), Variant::Float(r)) => l == r,
            (Variant::Err(l), Variant::Err(r)) => l == r,
            (Variant::List(l), Variant::List(r)) => l == r,
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
        Some(self.cmp(other))
    }
}

impl Ord for Var {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.variant(), other.variant()) {
            (Variant::None, Variant::None) => Ordering::Equal,
            (Variant::Str(l), Variant::Str(r)) => l.cmp(r),
            (Variant::Obj(l), Variant::Obj(r)) => l.cmp(r),
            (Variant::Int(l), Variant::Int(r)) => l.cmp(r),
            (Variant::Float(l), Variant::Float(r)) => R64::from(*l).cmp(&R64::from(*r)),
            (Variant::Err(l), Variant::Err(r)) => l.cmp(r),
            (Variant::List(l), Variant::List(r)) => l.cmp(r),
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
impl From<&i64> for Var {
    fn from(i: &i64) -> Self {
        v_int(*i)
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

impl From<Vec<Self>> for Var {
    fn from(l: Vec<Self>) -> Self {
        v_listv(l)
    }
}

impl<T, const COUNT: usize> From<[T; COUNT]> for Var
where
    for<'a> Var: From<&'a T>,
{
    fn from(a: [T; COUNT]) -> Self {
        v_list(&a.iter().map(|v| v.into()).collect::<Vec<_>>())
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

    use crate::var::error::Error;
    use crate::var::error::Error::{E_RANGE, E_TYPE};
    use crate::var::{v_empty_list, v_err, v_float, v_int, v_list, v_obj, v_str};

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
        assert_eq!(v_list(&[v_int(1), v_int(2)]).index(0), Ok(v_int(1)));
        assert_eq!(v_list(&[v_int(1), v_int(2)]).index(1), Ok(v_int(2)));
        assert_eq!(v_list(&[v_int(1), v_int(2)]).index(2), Ok(v_err(E_RANGE)));
        assert_eq!(v_str("ab").index(0), Ok(v_str("a")));
        assert_eq!(v_str("ab").index(1), Ok(v_str("b")));
        assert_eq!(v_str("ab").index(2), Ok(v_err(E_RANGE)));
    }

    #[test]
    fn test_eq() {
        assert_eq!(v_int(1), v_int(1));
        assert_eq!(v_float(1.), v_float(1.));
        assert_eq!(v_str("a"), v_str("a"));
        assert_eq!(v_str("a"), v_str("A"));
        assert_eq!(v_list(&[v_int(1), v_int(2)]), v_list(&[v_int(1), v_int(2)]));
        assert_eq!(v_obj(1), v_obj(1));
        assert_eq!(v_err(E_TYPE), v_err(E_TYPE));
    }

    #[test]
    fn test_ne() {
        assert_ne!(v_int(1), v_int(2));
        assert_ne!(v_float(1.), v_float(2.));
        assert_ne!(v_str("a"), v_str("b"));
        assert_ne!(v_list(&[v_int(1), v_int(2)]), v_list(&[v_int(1), v_int(3)]));
        assert_ne!(v_obj(1), v_obj(2));
        assert_ne!(v_err(E_TYPE), v_err(E_RANGE));
    }

    #[test]
    fn test_lt() {
        assert!(v_int(1) < v_int(2));
        assert!(v_float(1.) < v_float(2.));
        assert!(v_str("a") < v_str("b"));
        assert!(v_list(&[v_int(1), v_int(2)]) < v_list(&[v_int(1), v_int(3)]));
        assert!(v_obj(1) < v_obj(2));
        assert!(v_err(E_TYPE) < v_err(E_RANGE));
    }

    #[test]
    fn test_le() {
        assert!(v_int(1) <= v_int(2));
        assert!(v_float(1.) <= v_float(2.));
        assert!(v_str("a") <= v_str("b"));
        assert!(v_list(&[v_int(1), v_int(2)]) <= v_list(&[v_int(1), v_int(3)]));
        assert!(v_obj(1) <= v_obj(2));
        assert!(v_err(E_TYPE) <= v_err(E_RANGE));
    }

    #[test]
    fn test_gt() {
        assert!(v_int(2) > v_int(1));
        assert!(v_float(2.) > v_float(1.));
        assert!(v_str("b") > v_str("a"));
        assert!(v_list(&[v_int(1), v_int(3)]) > v_list(&[v_int(1), v_int(2)]));
        assert!(v_obj(2) > v_obj(1));
        assert!(v_err(E_RANGE) > v_err(E_TYPE));
    }

    #[test]
    fn test_ge() {
        assert!(v_int(2) >= v_int(1));
        assert!(v_float(2.) >= v_float(1.));
        assert!(v_str("b") >= v_str("a"));
        assert!(v_list(&[v_int(1), v_int(3)]) >= v_list(&[v_int(1), v_int(2)]));
        assert!(v_obj(2) >= v_obj(1));
        assert!(v_err(E_RANGE) >= v_err(E_TYPE));
    }

    #[test]
    fn test_partial_cmp() {
        assert_eq!(v_int(1).partial_cmp(&v_int(1)), Some(Ordering::Equal));
        assert_eq!(v_float(1.).partial_cmp(&v_float(1.)), Some(Ordering::Equal));
        assert_eq!(v_str("a").partial_cmp(&v_str("a")), Some(Ordering::Equal));
        assert_eq!(
            v_list(&[v_int(1), v_int(2)]).partial_cmp(&v_list(&[v_int(1), v_int(2)])),
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
            v_list(&[v_int(1), v_int(2)]).partial_cmp(&v_list(&[v_int(1), v_int(3)])),
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
            v_list(&[v_int(1), v_int(3)]).partial_cmp(&v_list(&[v_int(1), v_int(2)])),
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
            v_list(&[v_int(1), v_int(2)]).cmp(&v_list(&[v_int(1), v_int(2)])),
            Ordering::Equal
        );
        assert_eq!(v_obj(1).cmp(&v_obj(1)), Ordering::Equal);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_TYPE)), Ordering::Equal);

        assert_eq!(v_int(1).cmp(&v_int(2)), Ordering::Less);
        assert_eq!(v_float(1.).cmp(&v_float(2.)), Ordering::Less);
        assert_eq!(v_str("a").cmp(&v_str("b")), Ordering::Less);
        assert_eq!(
            v_list(&[v_int(1), v_int(2)]).cmp(&v_list(&[v_int(1), v_int(3)])),
            Ordering::Less
        );
        assert_eq!(v_obj(1).cmp(&v_obj(2)), Ordering::Less);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_RANGE)), Ordering::Less);

        assert_eq!(v_int(2).cmp(&v_int(1)), Ordering::Greater);
        assert_eq!(v_float(2.).cmp(&v_float(1.)), Ordering::Greater);
        assert_eq!(v_str("b").cmp(&v_str("a")), Ordering::Greater);
        assert_eq!(
            v_list(&[v_int(1), v_int(3)]).cmp(&v_list(&[v_int(1), v_int(2)])),
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
            v_list(&[v_int(1), v_int(2)])
                .partial_cmp(&v_list(&[v_int(1), v_int(2)]))
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
            v_list(&[v_int(1), v_int(2)])
                .partial_cmp(&v_list(&[v_int(1), v_int(3)]))
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
            v_list(&[v_int(1), v_int(3)])
                .partial_cmp(&v_list(&[v_int(1), v_int(2)]))
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
            v_list(&[v_int(1), v_int(2)]).cmp(&v_list(&[v_int(1), v_int(2)])),
            Ordering::Equal
        );
        assert_eq!(v_obj(1).cmp(&v_obj(1)), Ordering::Equal);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_TYPE)), Ordering::Equal);

        assert_eq!(v_int(1).cmp(&v_int(2)), Ordering::Less);
        assert_eq!(v_float(1.).cmp(&v_float(2.)), Ordering::Less);
        assert_eq!(v_str("a").cmp(&v_str("b")), Ordering::Less);
        assert_eq!(
            v_list(&[v_int(1), v_int(2)]).cmp(&v_list(&[v_int(1), v_int(3)])),
            Ordering::Less
        );
        assert_eq!(v_obj(1).cmp(&v_obj(2)), Ordering::Less);
        assert_eq!(v_err(E_TYPE).cmp(&v_err(E_RANGE)), Ordering::Less);

        assert_eq!(v_int(2).cmp(&v_int(1)), Ordering::Greater);
        assert_eq!(v_float(2.).cmp(&v_float(1.)), Ordering::Greater);
        assert_eq!(v_str("b").cmp(&v_str("a")), Ordering::Greater);
        assert_eq!(
            v_list(&[v_int(1), v_int(3)]).cmp(&v_list(&[v_int(1), v_int(2)])),
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
        assert!(v_list(&[v_int(1), v_int(2)]).is_true());
        assert!(!v_obj(1).is_true());
        assert!(!v_err(E_TYPE).is_true());
    }

    #[test]
    fn test_listrangeset() {
        let base = v_list(&[v_int(1), v_int(2), v_int(3), v_int(4)]);

        // {1,2,3,4}[1..2] = {"a", "b", "c"} => {1, "a", "b", "c", 4}
        let value = v_list(&[v_str("a"), v_str("b"), v_str("c")]);
        let expected = v_list(&[v_int(1), v_str("a"), v_str("b"), v_str("c"), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {"a"} => {1, "a", 4}
        let value = v_list(&[v_str("a")]);
        let expected = v_list(&[v_int(1), v_str("a"), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {} => {1,4}
        let value = v_empty_list();
        let expected = v_list(&[v_int(1), v_int(4)]);
        assert_eq!(base.rangeset(value, 2, 3).unwrap(), expected);

        // {1,2,3,4}[1..2] = {"a", "b"} => {1, "a", "b", 4}
        let value = v_list(&[v_str("a"), v_str("b")]);
        let expected = v_list(&[v_int(1), v_str("a"), v_str("b"), v_int(4)]);
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
    fn test_range() -> Result<(), Error> {
        // test on integer list
        let int_list = v_list(&[1.into(), 2.into(), 3.into(), 4.into(), 5.into()]);
        assert_eq!(
            int_list.range(2, 4)?,
            v_list(&[2.into(), 3.into(), 4.into()])
        );

        // test on string
        let string = v_str("hello world");
        assert_eq!(string.range(2, 7)?, v_str("ello w"));

        // range with upper higher than lower, moo returns empty list for this (!)
        let empty_list = v_empty_list();
        assert_eq!(empty_list.range(1, 0), Ok(v_empty_list()));
        // test on out of range
        let int_list = v_list(&[1.into(), 2.into(), 3.into()]);
        assert_eq!(int_list.range(2, 4), Ok(v_err(E_RANGE)));
        // test on type mismatch
        let var_int = v_int(10);
        assert_eq!(var_int.range(1, 5), Ok(v_err(E_TYPE)));

        Ok(())
    }
}
