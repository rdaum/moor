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

use crate::var::list::List;
use crate::var::variant::Variant;
use crate::var::Error::{E_INVARG, E_RANGE, E_TYPE};
use crate::var::{map, Flyweight, IndexMode, Sequence, TypeClass};
use crate::var::{string, Associative};
use crate::var::{Error, Obj, VarType};
use crate::{BincodeAsByteBufferExt, Symbol};
use bincode::{Decode, Encode};
use std::cmp::{min, Ordering};
use std::fmt::{Debug, Formatter};
use std::hash::Hash;

#[derive(Clone, Encode, Decode)]
pub struct Var(Variant);

impl BincodeAsByteBufferExt for Var {}

impl Debug for Var {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.variant())
    }
}

impl Var {
    pub fn from_variant(variant: Variant) -> Self {
        Var(variant)
    }

    pub fn mk_integer(i: i64) -> Self {
        let v = Variant::Int(i);
        Var(v)
    }

    pub fn mk_none() -> Self {
        Var(Variant::None)
    }

    pub fn mk_str(s: &str) -> Self {
        Var(Variant::Str(string::Str::mk_str(s)))
    }

    pub fn mk_float(f: f64) -> Self {
        Var(Variant::Float(f))
    }

    pub fn mk_error(e: Error) -> Self {
        Var(Variant::Err(e))
    }

    pub fn mk_object(o: Obj) -> Self {
        Var(Variant::Obj(o))
    }

    pub fn type_code(&self) -> VarType {
        match self.variant() {
            Variant::Int(_) => VarType::TYPE_INT,
            Variant::Obj(_) => VarType::TYPE_OBJ,
            Variant::Str(_) => VarType::TYPE_STR,
            Variant::Err(_) => VarType::TYPE_ERR,
            Variant::List(_) => VarType::TYPE_LIST,
            Variant::None => VarType::TYPE_NONE,
            Variant::Float(_) => VarType::TYPE_FLOAT,
            Variant::Map(_) => VarType::TYPE_MAP,
            Variant::Flyweight(_) => VarType::TYPE_FLYWEIGHT,
        }
    }

    pub fn mk_list(values: &[Var]) -> Self {
        List::build(values)
    }

    pub fn mk_list_iter<IT: IntoIterator<Item = Var>>(values: IT) -> Self {
        Var::from_iter(values)
    }

    pub fn mk_map(pairs: &[(Var, Var)]) -> Self {
        map::Map::build(pairs.iter())
    }

    pub fn mk_map_iter<'a, I: Iterator<Item = &'a (Var, Var)>>(pairs: I) -> Self {
        map::Map::build(pairs)
    }

    pub fn variant(&self) -> &Variant {
        &self.0
    }

    pub fn is_true(&self) -> bool {
        match self.variant() {
            Variant::None => false,
            Variant::Obj(_) => false,
            Variant::Int(i) => *i != 0,
            Variant::Float(f) => *f != 0.0,
            Variant::List(l) => !l.is_empty(),
            Variant::Str(s) => !s.is_empty(),
            Variant::Map(m) => !m.is_empty(),
            Variant::Err(_) => false,
            Variant::Flyweight(f) => !f.is_empty(),
        }
    }

    /// 0-based index into a sequence type, or map lookup by key
    /// If not a sequence, returns Err(E_INVARG)
    /// For strings and lists, the index must be a positive integer, or Err(E_TYPE)
    /// Range errors are Err(E_RANGE)
    /// Otherwise returns the value
    pub fn index(&self, index: &Var, index_mode: IndexMode) -> Result<Self, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let idx = match index.variant() {
                    Variant::Int(i) => {
                        let i = index_mode.adjust_i64(*i);
                        if i < 0 {
                            return Err(E_RANGE);
                        }
                        i as usize
                    }
                    _ => {
                        return Err(E_TYPE);
                    }
                };

                if idx >= s.len() {
                    return Err(E_RANGE);
                }

                s.index(idx)
            }
            TypeClass::Associative(a) => a.index(index),
            TypeClass::Scalar => Err(E_TYPE),
        }
    }

    /// Assign a new value to `index`nth element of the sequence, or to a key in an associative type.
    pub fn index_set(
        &self,
        idx: &Self,
        value: &Self,
        index_mode: IndexMode,
    ) -> Result<Self, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let idx = match idx.variant() {
                    Variant::Int(i) => {
                        let i = index_mode.adjust_i64(*i);

                        if i < 0 {
                            return Err(E_RANGE);
                        }
                        i as usize
                    }
                    _ => return Err(E_INVARG),
                };
                s.index_set(idx, value)
            }
            TypeClass::Associative(a) => a.index_set(idx, value),
            TypeClass::Scalar => Err(E_TYPE),
        }
    }

    /// Insert a new value at `index` in a sequence only.
    /// If the value is not a sequence, returns Err(E_INVARG).
    /// To add a value to a map use `index_set`.
    /// If the index is negative, it is treated as 0.
    pub fn insert(&self, index: &Var, value: &Var, index_mode: IndexMode) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let index = match index.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => return Err(E_INVARG),
                };
                let index = if index < 0 {
                    0
                } else {
                    min(index as usize, s.len())
                };

                if index > s.len() {
                    return Err(E_RANGE);
                }

                s.insert(index, value)
            }
            _ => Err(E_TYPE),
        }
    }

    pub fn range(&self, from: &Var, to: &Var, index_mode: IndexMode) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let from = match from.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => return Err(E_INVARG),
                };

                let to = match to.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => return Err(E_INVARG),
                };

                s.range(from, to)
            }
            TypeClass::Associative(a) => a.range(from, to),
            TypeClass::Scalar => Err(E_TYPE),
        }
    }

    pub fn range_set(
        &self,
        from: &Var,
        to: &Var,
        with: &Var,
        index_mode: IndexMode,
    ) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let from = match from.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => return Err(E_INVARG),
                };

                let to = match to.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => return Err(E_INVARG),
                };

                s.range_set(from, to, with)
            }
            TypeClass::Associative(a) => a.range_set(from, to, with),
            TypeClass::Scalar => Err(E_TYPE),
        }
    }

    pub fn append(&self, other: &Var) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => s.append(other),
            _ => Err(E_TYPE),
        }
    }

    pub fn push(&self, value: &Var) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => s.push(value),
            _ => Err(E_TYPE),
        }
    }

    pub fn contains(&self, value: &Var, case_sensitive: bool) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let c = s.contains(value, case_sensitive)?;
                Ok(v_bool(c))
            }
            TypeClass::Associative(a) => {
                let c = a.contains_key(value, case_sensitive)?;
                Ok(v_bool(c))
            }
            TypeClass::Scalar => Err(E_INVARG),
        }
    }

    pub fn index_in(
        &self,
        value: &Var,
        case_sensitive: bool,
        index_mode: IndexMode,
    ) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let idx = s
                    .index_in(value, case_sensitive)?
                    .map(|i| i as i64)
                    .unwrap_or(-1);
                Ok(v_int(index_mode.reverse_adjust_isize(idx as isize) as i64))
            }
            TypeClass::Associative(a) => {
                let idx = a
                    .index_in(value, case_sensitive)?
                    .map(|i| i as i64)
                    .unwrap_or(-1);
                Ok(v_int(index_mode.reverse_adjust_isize(idx as isize) as i64))
            }
            _ => Err(E_INVARG),
        }
    }

    pub fn remove_at(&self, index: &Var, index_mode: IndexMode) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let index = match index.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => return Err(E_INVARG),
                };

                if index < 0 {
                    return Err(E_RANGE);
                }

                s.remove_at(index as usize)
            }
            _ => Err(E_TYPE),
        }
    }

    pub fn remove(&self, value: &Var, case_sensitive: bool) -> Result<(Var, Option<Var>), Error> {
        match self.type_class() {
            TypeClass::Associative(a) => Ok(a.remove(value, case_sensitive)),
            _ => Err(E_INVARG),
        }
    }

    pub fn is_sequence(&self) -> bool {
        self.type_class().is_sequence()
    }

    pub fn is_associative(&self) -> bool {
        self.type_class().is_associative()
    }

    pub fn is_scalar(&self) -> bool {
        self.type_class().is_scalar()
    }

    pub fn is_string(&self) -> bool {
        matches!(self.variant(), Variant::Str(_))
    }

    pub fn is_empty(&self) -> Result<bool, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => Ok(s.is_empty()),
            TypeClass::Associative(a) => Ok(a.is_empty()),
            TypeClass::Scalar => Err(E_INVARG),
        }
    }

    pub fn eq_case_sensitive(&self, other: &Var) -> bool {
        match (self.variant(), other.variant()) {
            (Variant::Str(s1), Variant::Str(s2)) => s1.as_string() == s2.as_string(),
            (Variant::List(l1), Variant::List(l2)) => {
                if l1.len() != l2.len() {
                    return false;
                }
                let left_items = l1.iter();
                let right_items = l2.iter();
                for (left, right) in left_items.zip(right_items) {
                    if !left.eq_case_sensitive(&right) {
                        return false;
                    }
                }
                true
            }
            (Variant::Map(m1), Variant::Map(m2)) => {
                if m1.len() != m2.len() {
                    return false;
                }
                let left_pairs = m1.iter();
                let right_pairs = m2.iter();
                for (left, right) in left_pairs.zip(right_pairs) {
                    if !left.0.eq_case_sensitive(&right.0) || !left.1.eq_case_sensitive(&right.1) {
                        return false;
                    }
                }
                true
            }
            _ => self.eq(other),
        }
    }

    pub fn cmp_case_sensitive(&self, other: &Var) -> Ordering {
        match (self.variant(), other.variant()) {
            (Variant::Str(s1), Variant::Str(s2)) => s1.as_string().cmp(s2.as_string()),
            _ => self.cmp(other),
        }
    }

    pub fn len(&self) -> Result<usize, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => Ok(s.len()),
            TypeClass::Associative(a) => Ok(a.len()),
            TypeClass::Scalar => Err(E_INVARG),
        }
    }

    pub fn type_class(&self) -> TypeClass {
        match self.variant() {
            Variant::List(s) => TypeClass::Sequence(s),
            Variant::Flyweight(f) => TypeClass::Sequence(f),
            Variant::Str(s) => TypeClass::Sequence(s),
            Variant::Map(m) => TypeClass::Associative(m),
            _ => TypeClass::Scalar,
        }
    }
}

pub fn v_int(i: i64) -> Var {
    Var::mk_integer(i)
}

pub fn v_bool(b: bool) -> Var {
    if b {
        v_int(1)
    } else {
        v_int(0)
    }
}

pub fn v_none() -> Var {
    // TODO lazy_static singleton
    Var::mk_none()
}

pub fn v_str(s: &str) -> Var {
    Var::mk_str(s)
}

pub fn v_string(s: String) -> Var {
    Var::mk_str(&s)
}

pub fn v_list(values: &[Var]) -> Var {
    Var::mk_list(values)
}

pub fn v_list_iter<IT: IntoIterator<Item = Var>>(values: IT) -> Var {
    Var::mk_list_iter(values)
}

pub fn v_map(pairs: &[(Var, Var)]) -> Var {
    Var::mk_map(pairs)
}

pub fn v_map_iter<'a, I: Iterator<Item = &'a (Var, Var)>>(pairs: I) -> Var {
    Var::mk_map_iter(pairs)
}

pub fn v_float(f: f64) -> Var {
    Var::mk_float(f)
}

pub fn v_err(e: Error) -> Var {
    Var::mk_error(e)
}

pub fn v_objid(o: i32) -> Var {
    Var::mk_object(Obj::mk_id(o))
}

pub fn v_obj(o: Obj) -> Var {
    Var::mk_object(o)
}

pub fn v_flyweight(
    delegate: Obj,
    slots: &[(Symbol, Var)],
    contents: List,
    seal: Option<String>,
) -> Var {
    let fl = Flyweight::mk_flyweight(delegate, slots, contents, seal);
    Var::from_variant(Variant::Flyweight(fl))
}

pub fn v_empty_list() -> Var {
    // TODO: lazy static
    v_list(&[])
}

pub fn v_empty_str() -> Var {
    // TODO: lazy static
    v_str("")
}

pub fn v_empty_map() -> Var {
    // TODO: lazy static
    v_map(&[])
}

impl From<i64> for Var {
    fn from(i: i64) -> Self {
        Var::mk_integer(i)
    }
}

impl From<&str> for Var {
    fn from(s: &str) -> Self {
        Var::mk_str(s)
    }
}

impl From<String> for Var {
    fn from(s: String) -> Self {
        Var::mk_str(&s)
    }
}

impl From<Obj> for Var {
    fn from(o: Obj) -> Self {
        Var::mk_object(o)
    }
}

impl From<Error> for Var {
    fn from(e: Error) -> Self {
        Var::mk_error(e)
    }
}

impl PartialEq<Self> for Var {
    fn eq(&self, other: &Self) -> bool {
        self.variant() == other.variant()
    }
}

impl Eq for Var {}

impl Ord for Var {
    fn cmp(&self, other: &Self) -> Ordering {
        self.variant().cmp(other.variant())
    }
}

impl PartialOrd<Self> for Var {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Hash for Var {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.variant().hash(state)
    }
}

#[cfg(test)]
mod tests {
    use crate::var::var::Var;
    use crate::var::variant::Variant;

    #[test]
    fn test_int_pack_unpack() {
        let i = Var::mk_integer(42);

        match i.variant() {
            Variant::Int(i) => assert_eq!(*i, 42),
            _ => panic!("Expected integer"),
        }
    }

    #[test]
    fn test_float_pack_unpack() {
        let f = Var::mk_float(42.0);

        match f.variant() {
            Variant::Float(f) => assert_eq!(*f, 42.0),
            _ => panic!("Expected float"),
        }
    }

    #[test]
    fn test_alpha_numeric_sort_order() {
        // "a" should come after "6"
        let six = Var::mk_integer(6);
        let a = Var::mk_str("a");
        assert_eq!(six.cmp(&a), std::cmp::Ordering::Less);

        // and 9 before "a" as well
        let nine = Var::mk_integer(9);
        assert_eq!(nine.cmp(&a), std::cmp::Ordering::Less);

        // now in the other order.
        assert_eq!(a.cmp(&six), std::cmp::Ordering::Greater);
        assert_eq!(a.cmp(&nine), std::cmp::Ordering::Greater);
    }
}
