// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

use crate::Associative;
use crate::error::ErrorCode;
use crate::error::ErrorCode::{E_INVARG, E_RANGE, E_TYPE};
use crate::list::List;
use crate::variant::Variant;
use crate::{BincodeAsByteBufferExt, Symbol};
use crate::{Error, Obj, VarType};
use crate::{Flyweight, IndexMode, Sequence, TypeClass, map};
use bincode::{Decode, Encode};
use std::cmp::{Ordering, min};
use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::sync::Arc;

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
        Var(Variant::Str(s.into()))
    }

    pub fn mk_string(s: String) -> Self {
        Var(Variant::Str(s.into()))
    }

    pub fn mk_float(f: f64) -> Self {
        Var(Variant::Float(f))
    }

    pub fn mk_error(e: Error) -> Self {
        Var(Variant::Err(Arc::new(e)))
    }

    pub fn mk_object(o: Obj) -> Self {
        Var(Variant::Obj(o))
    }

    pub fn mk_bool(b: bool) -> Self {
        Var(Variant::Bool(b))
    }

    pub fn mk_symbol(s: Symbol) -> Self {
        Var(Variant::Sym(s))
    }

    pub fn mk_binary(bytes: Vec<u8>) -> Self {
        use crate::binary::Binary;
        Var(Variant::Binary(Box::new(Binary::from_bytes(bytes))))
    }

    pub fn mk_lambda(
        params: crate::program::opcode::ScatterArgs,
        body: crate::program::program::Program,
        captured_env: Vec<Vec<Var>>,
        self_var: Option<crate::program::names::Name>,
    ) -> Self {
        use crate::lambda::Lambda;
        Var(Variant::Lambda(Box::new(Lambda::new(
            params,
            body,
            captured_env,
            self_var,
        ))))
    }

    pub fn type_code(&self) -> VarType {
        match self.variant() {
            Variant::Bool(_) => VarType::TYPE_BOOL,
            Variant::Int(_) => VarType::TYPE_INT,
            Variant::Obj(_) => VarType::TYPE_OBJ,
            Variant::Str(_) => VarType::TYPE_STR,
            Variant::Err(_) => VarType::TYPE_ERR,
            Variant::List(_) => VarType::TYPE_LIST,
            Variant::None => VarType::TYPE_NONE,
            Variant::Float(_) => VarType::TYPE_FLOAT,
            Variant::Map(_) => VarType::TYPE_MAP,
            Variant::Flyweight(_) => VarType::TYPE_FLYWEIGHT,
            Variant::Sym(_) => VarType::TYPE_SYMBOL,
            Variant::Binary(_) => VarType::TYPE_BINARY,
            Variant::Lambda(_) => VarType::TYPE_LAMBDA,
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

    /// Extract the integer value if this is an integer variant, otherwise None.
    pub fn as_integer(&self) -> Option<i64> {
        match self.variant() {
            Variant::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Extract the string value if this is a string variant, otherwise None.
    pub fn as_string(&self) -> Option<&str> {
        match self.variant() {
            Variant::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Extract the boolean value if this is a boolean variant, otherwise None.
    pub fn as_bool(&self) -> Option<bool> {
        match self.variant() {
            Variant::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Extract the object value if this is an object variant, otherwise None.
    pub fn as_object(&self) -> Option<Obj> {
        match self.variant() {
            Variant::Obj(o) => Some(*o),
            _ => None,
        }
    }

    /// Extract the float value if this is a float variant, otherwise None.
    pub fn as_float(&self) -> Option<f64> {
        match self.variant() {
            Variant::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// Extract the list value if this is a list variant, otherwise None.
    pub fn as_list(&self) -> Option<&List> {
        match self.variant() {
            Variant::List(l) => Some(l),
            _ => None,
        }
    }

    /// Extract the map value if this is a map variant, otherwise None.
    pub fn as_map(&self) -> Option<&map::Map> {
        match self.variant() {
            Variant::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Extract the error value if this is an error variant, otherwise None.
    pub fn as_error(&self) -> Option<&Error> {
        match self.variant() {
            Variant::Err(e) => Some(e.as_ref()),
            _ => None,
        }
    }

    /// Extract the flyweight value if this is a flyweight variant, otherwise None.
    pub fn as_flyweight(&self) -> Option<&Flyweight> {
        match self.variant() {
            Variant::Flyweight(f) => Some(f),
            _ => None,
        }
    }

    /// Extract the symbol value if this is a symbol variant, otherwise None.
    pub fn as_sym(&self) -> Option<Symbol> {
        match self.variant() {
            Variant::Sym(s) => Some(*s),
            _ => None,
        }
    }

    /// Extract the binary value if this is a binary variant, otherwise None.
    pub fn as_binary(&self) -> Option<&crate::Binary> {
        match self.variant() {
            Variant::Binary(b) => Some(b.as_ref()),
            _ => None,
        }
    }

    /// Extract the lambda value if this is a lambda variant, otherwise None.
    pub fn as_lambda(&self) -> Option<&crate::Lambda> {
        match self.variant() {
            Variant::Lambda(l) => Some(l.as_ref()),
            _ => None,
        }
    }

    /// Returns true if this is a None variant.
    pub fn is_none(&self) -> bool {
        matches!(self.variant(), Variant::None)
    }

    /// If a string, turn into symbol, or if already a symbol, return that.
    /// Otherwise, E_TYPE
    pub fn as_symbol(&self) -> Result<Symbol, Error> {
        match self.variant() {
            Variant::Str(s) => Ok(Symbol::mk(s.as_str())),
            Variant::Sym(s) => Ok(*s),
            Variant::Err(e) => Ok(e.name()),
            _ => Err(E_TYPE.with_msg(|| {
                format!("Cannot convert {} to symbol", self.type_code().to_literal())
            })),
        }
    }

    pub fn is_true(&self) -> bool {
        match self.variant() {
            Variant::None => false,
            Variant::Bool(b) => *b,
            Variant::Obj(_) => false,
            Variant::Int(i) => *i != 0,
            Variant::Float(f) => *f != 0.0,
            Variant::List(l) => !l.is_empty(),
            Variant::Str(s) => !s.is_empty(),
            Variant::Map(m) => !m.is_empty(),
            Variant::Err(_) => false,
            Variant::Flyweight(f) => !f.is_empty(),
            Variant::Sym(_) => true,
            Variant::Binary(b) => !b.is_empty(),
            Variant::Lambda(_) => true,
        }
    }

    /// Index into a sequence type, or get Nth element of an association set
    /// If not a sequence, or association, returns Err(E_INVARG)
    /// The index must be a positive integer, or Err(E_TYPE).
    /// Associations return the key-value pair.
    /// Range errors are Err(E_RANGE)
    /// Otherwise returns the value
    pub fn index(&self, index: &Var, index_mode: IndexMode) -> Result<Self, Error> {
        let tc = self.type_class();
        if tc.is_scalar() {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot index into scalar value {}",
                    self.type_code().to_literal()
                )
            }));
        }
        let idx = match index.variant() {
            Variant::Int(i) => {
                let i = index_mode.adjust_i64(*i);
                if i < 0 {
                    return Err(E_RANGE.with_msg(|| {
                        format!("Cannot index into sequence with negative index {i}")
                    }));
                }
                i as usize
            }
            _ => {
                return Err(E_TYPE.with_msg(|| {
                    format!(
                        "Cannot index into sequence with non-integer index {}",
                        index.type_code().to_literal()
                    )
                }));
            }
        };

        match tc {
            TypeClass::Sequence(s) => {
                let value = s.index(idx)?;
                Ok(value)
            }
            TypeClass::Associative(a) => {
                let value = a.index(idx)?;
                Ok(value.1)
            }
            _ => Err(E_TYPE
                .with_msg(|| format!("Cannot index into type {}", self.type_code().to_literal()))),
        }
    }

    /// Return the associative key at `key`, or the Nth element of a sequence.
    /// If not a sequence or associative, returns Err(E_INVARG)
    /// For strings and lists, the index must be a positive integer, or Err(E_TYPE)
    /// Range errors are Err(E_RANGE)
    /// Otherwise returns the value
    pub fn get(&self, key: &Var, index_mode: IndexMode) -> Result<Self, Error> {
        match self.type_class() {
            TypeClass::Sequence(_) => {
                let value = self.index(key, index_mode)?;
                Ok(value)
            }
            TypeClass::Associative(a) => {
                let value = a.get(key)?;
                Ok(value)
            }
            _ => Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot index value from type {}",
                    self.type_code().to_literal()
                )
            })),
        }
    }

    /// Update the associative key at `key` to `value` and return the modification.
    pub fn set(&self, key: &Var, value: &Var, index_mode: IndexMode) -> Result<Self, Error> {
        match self.type_class() {
            TypeClass::Sequence(_) => {
                let value = self.index_set(key, value, index_mode)?;
                Ok(value)
            }
            TypeClass::Associative(s) => s.set(key, value),
            _ => Err(E_TYPE.with_msg(|| {
                format!("Cannot set value in type {}", self.type_code().to_literal())
            })),
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
                            return Err(E_RANGE.with_msg(|| {
                                format!("Cannot index into sequence with negative index {i}")
                            }));
                        }
                        i as usize
                    }
                    _ => {
                        return Err(E_INVARG.with_msg(|| {
                            format!(
                                "Cannot index into sequence with non-integer index {}",
                                idx.type_code().to_literal()
                            )
                        }));
                    }
                };
                s.index_set(idx, value)
            }
            _ => Err(E_TYPE.with_msg(|| {
                format!("Cannot set value in type {}", self.type_code().to_literal())
            })),
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
                    _ => {
                        return Err(E_INVARG.with_msg(|| {
                            format!(
                                "Cannot insert into sequence with non-integer index {}",
                                index.type_code().to_literal()
                            )
                        }));
                    }
                };
                let index = if index < 0 {
                    0
                } else {
                    min(index as usize, s.len())
                };

                if index > s.len() {
                    return Err(E_RANGE.with_msg(|| {
                        format!(
                            "Cannot insert into sequence with index {} greater than length {}",
                            index,
                            s.len()
                        )
                    }));
                }

                s.insert(index, value)
            }
            _ => Err(E_TYPE
                .with_msg(|| format!("Cannot insert into type {}", self.type_code().to_literal()))),
        }
    }

    pub fn range(&self, from: &Var, to: &Var, index_mode: IndexMode) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let from = match from.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => {
                        return Err(E_INVARG.with_msg(|| {
                            format!(
                                "Cannot index into sequence with non-integer index {}",
                                from.type_code().to_literal()
                            )
                        }));
                    }
                };

                let to = match to.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => {
                        return Err(E_INVARG.with_msg(|| {
                            format!(
                                "Cannot index into sequence with non-integer index {}",
                                to.type_code().to_literal()
                            )
                        }));
                    }
                };

                s.range(from, to)
            }
            TypeClass::Associative(a) => a.range(from, to),
            TypeClass::Scalar => Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot index into scalar value {}",
                    self.type_code().to_literal()
                )
            })),
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
                    _ => {
                        return Err(E_INVARG.with_msg(|| {
                            format!(
                                "Cannot index into sequence with non-integer index {}",
                                from.type_code().to_literal()
                            )
                        }));
                    }
                };

                let to = match to.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => {
                        return Err(E_INVARG.with_msg(|| {
                            format!(
                                "Cannot index into sequence with non-integer index {}",
                                to.type_code().to_literal()
                            )
                        }));
                    }
                };

                s.range_set(from, to, with)
            }
            TypeClass::Associative(a) => a.range_set(from, to, with),
            TypeClass::Scalar => Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot index into scalar value {}",
                    self.type_code().to_literal()
                )
            })),
        }
    }

    pub fn append(&self, other: &Var) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => s.append(other),
            _ => Err(E_TYPE
                .with_msg(|| format!("Cannot append to type {}", self.type_code().to_literal()))),
        }
    }

    pub fn push(&self, value: &Var) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => s.push(value),
            _ => Err(E_TYPE
                .with_msg(|| format!("Cannot push to type {}", self.type_code().to_literal()))),
        }
    }

    pub fn contains(&self, value: &Var, case_sensitive: bool) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let c = s.contains(value, case_sensitive)?;
                Ok(v_bool_int(c))
            }
            TypeClass::Associative(a) => {
                let c = a.contains_key(value, case_sensitive)?;
                Ok(v_bool_int(c))
            }
            TypeClass::Scalar => Err(E_INVARG.with_msg(|| {
                format!(
                    "Cannot check for membership in scalar value {}",
                    self.type_code().to_literal()
                )
            })),
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
            _ => Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot check for membership in type {}",
                    self.type_code().to_literal()
                )
            })),
        }
    }

    pub fn remove_at(&self, index: &Var, index_mode: IndexMode) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let index = match index.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(*i),
                    _ => {
                        return Err(E_INVARG.with_msg(|| {
                            format!(
                                "Cannot index into sequence with non-integer index {}",
                                index.type_code().to_literal()
                            )
                        }));
                    }
                };

                if index < 0 {
                    return Err(E_RANGE.with_msg(|| {
                        format!("Cannot index into sequence with negative index {index}")
                    }));
                }

                s.remove_at(index as usize)
            }
            _ => Err(E_TYPE
                .with_msg(|| format!("Cannot remove from type {}", self.type_code().to_literal()))),
        }
    }

    pub fn remove(&self, value: &Var, case_sensitive: bool) -> Result<(Var, Option<Var>), Error> {
        match self.type_class() {
            TypeClass::Associative(a) => Ok(a.remove(value, case_sensitive)),
            _ => Err(E_INVARG
                .with_msg(|| format!("Cannot remove from type {}", self.type_code().to_literal()))),
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
            TypeClass::Scalar => Err(E_INVARG.with_msg(|| {
                format!(
                    "Cannot check if scalar value {} is empty",
                    self.type_code().to_literal()
                )
            })),
        }
    }

    pub fn eq_case_sensitive(&self, other: &Var) -> bool {
        match (self.variant(), other.variant()) {
            (Variant::Str(s1), Variant::Str(s2)) => s1.as_str() == s2.as_str(),
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
            (Variant::Str(s1), Variant::Str(s2)) => s1.as_str().cmp(s2.as_str()),
            _ => self.cmp(other),
        }
    }

    pub fn len(&self) -> Result<usize, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => Ok(s.len()),
            TypeClass::Associative(a) => Ok(a.len()),
            TypeClass::Scalar => Err(E_INVARG.with_msg(|| {
                format!(
                    "Cannot get length of scalar value {}",
                    self.type_code().to_literal()
                )
            })),
        }
    }

    pub fn type_class(&self) -> TypeClass<'_> {
        match self.variant() {
            Variant::List(s) => TypeClass::Sequence(s),
            Variant::Flyweight(f) => TypeClass::Sequence(f),
            Variant::Str(s) => TypeClass::Sequence(s),
            Variant::Binary(b) => TypeClass::Sequence(b.as_ref()),
            Variant::Map(m) => TypeClass::Associative(m),
            _ => TypeClass::Scalar,
        }
    }
}

pub fn v_int(i: i64) -> Var {
    Var::mk_integer(i)
}

/// Produces a truthy integer, not a Variant::Bool boolean value in order to maintain
/// backwards compatibility with LambdaMOO cores.
pub fn v_bool_int(b: bool) -> Var {
    if b { v_int(1) } else { v_int(0) }
}

pub fn v_bool(b: bool) -> Var {
    Var::mk_bool(b)
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

pub fn v_arc_string(s: std::sync::Arc<String>) -> Var {
    let str_val = crate::string::Str::mk_arc_str(s);
    Var::from_variant(crate::variant::Variant::Str(str_val))
}

#[cfg(test)]
mod v_arc_string_tests {
    use super::*;
    use crate::variant::Variant;

    #[test]
    fn test_v_arc_string() {
        let arc_string = std::sync::Arc::new("test_string".to_string());
        let var = v_arc_string(arc_string.clone());

        match var.variant() {
            Variant::Str(s) => {
                assert_eq!(s.as_str(), "test_string");
                assert_eq!(s.as_arc_string().as_ref(), arc_string.as_ref());
                // Verify it's actually sharing the Arc (same pointer)
                assert!(std::sync::Arc::ptr_eq(&s.as_arc_string(), &arc_string));
            }
            _ => panic!("Expected string variant"),
        }
    }
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

pub fn v_err(e: ErrorCode) -> Var {
    Var::mk_error(e.into())
}

pub fn v_error(e: Error) -> Var {
    Var::mk_error(e)
}

pub fn v_objid(o: i32) -> Var {
    Var::mk_object(Obj::mk_id(o))
}

pub fn v_obj(o: Obj) -> Var {
    Var::mk_object(o)
}

pub fn v_sym(s: impl Into<Symbol>) -> Var {
    Var::mk_symbol(s.into())
}

pub fn v_binary(bytes: Vec<u8>) -> Var {
    Var::mk_binary(bytes)
}

pub fn v_flyweight(delegate: Obj, slots: &[(Symbol, Var)], contents: List) -> Var {
    let fl = Flyweight::mk_flyweight(delegate, slots, contents);
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

impl From<Vec<u8>> for Var {
    fn from(bytes: Vec<u8>) -> Self {
        Var::mk_binary(bytes)
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
    use crate::var::Var;
    use crate::variant::Variant;

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

    #[test]
    fn test_var_size() {
        // Ensure that Var never exceeds 128-bits
        assert!(
            size_of::<Var>() <= 16,
            "Var size exceeds 128 bits: {}",
            size_of::<Var>()
        );
    }
}
