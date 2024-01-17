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

use std::cmp::min;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::ops::Index;

use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use im::Vector;

use crate::var::variant::Variant;
use crate::var::{v_empty_list, Var};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct List {
    pub(crate) inner: Vector<Var>,
}

impl Encode for List {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        // Encode length
        let len = self.inner.len();
        len.encode(encoder)?;
        for item in self.inner.iter() {
            item.encode(encoder)?;
        }
        Ok(())
    }
}

impl Decode for List {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let len = usize::decode(decoder)?;
        let mut items = Vector::new();
        for _ in 0..len {
            items.push_back(Var::decode(decoder)?);
        }
        Ok(Self::from_imvec(items))
    }
}

impl<'a> BorrowDecode<'a> for List {
    fn borrow_decode<D: BorrowDecoder<'a>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let len = usize::decode(decoder)?;
        let mut items = Vector::new();
        for _ in 0..len {
            items.push_back(Var::borrow_decode(decoder)?);
        }
        Ok(Self::from_imvec(items))
    }
}
impl List {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Vector::new(),
        }
    }

    #[must_use]
    pub fn from_slice(vec: &[Var]) -> Self {
        Self { inner: vec.into() }
    }

    pub fn from_imvec(vec: im::vector::Vector<Var>) -> Self {
        Self { inner: vec }
    }

    #[must_use]
    pub fn push(&self, v: &Var) -> Var {
        let mut new_list = self.inner.clone();
        new_list.push_back(v.clone());
        Variant::List(Self::from_imvec(new_list)).into()
    }

    /// Take the first item from the front, and return (item, `new_list`)
    #[must_use]
    pub fn pop_front(&self) -> (Var, Var) {
        if self.inner.is_empty() {
            return (v_empty_list(), v_empty_list());
        }
        let mut new_list = self.inner.clone();
        let item = new_list.remove(0);
        (
            item.clone(),
            Variant::List(Self::from_imvec(new_list)).into(),
        )
    }

    #[must_use]
    pub fn append(&self, other: &Self) -> Var {
        let mut new_list = self.inner.clone();
        new_list.append(other.inner.clone());
        Variant::List(Self::from_imvec(new_list)).into()
    }

    #[must_use]
    pub fn remove_at(&self, index: usize) -> Var {
        let mut new_list = self.inner.clone();
        new_list.remove(index);
        Variant::List(Self::from_imvec(new_list)).into()
    }

    /// Remove the first found instance of the given value from the list.
    #[must_use]
    pub fn setremove(&self, value: &Var) -> Var {
        if self.inner.is_empty() {
            return v_empty_list();
        }
        let mut new_list = Vector::new();
        let mut found = false;
        for v in self.inner.iter() {
            if !found && v == value {
                found = true;
                continue;
            }
            new_list.push_back(v.clone());
        }
        Variant::List(Self::from_imvec(new_list)).into()
    }

    #[must_use]
    pub fn insert(&self, index: isize, v: &Var) -> Var {
        let index = if index < 0 {
            0
        } else {
            min(index as usize, self.inner.len())
        };
        let mut new_list = self.inner.clone();
        let mut new_list = new_list.slice(..index).clone();
        new_list.push_back(v.clone());
        new_list.append(self.inner.clone().slice(index..));
        Variant::List(Self::from_imvec(new_list)).into()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    // "in" operator is case insensitive...
    #[must_use]
    pub fn contains(&self, v: &Var) -> bool {
        self.inner.contains(v)
    }

    // but bf_is_member is not... sigh.
    #[must_use]
    pub fn contains_case_sensitive(&self, v: &Var) -> bool {
        if let Variant::Str(s) = v.variant() {
            for item in self.inner.iter() {
                if let Variant::Str(s2) = item.variant() {
                    if s.as_str() == s2.as_str() {
                        return true;
                    }
                }
            }
            return false;
        }
        self.inner.contains(v)
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Var> {
        self.inner.get(index)
    }

    #[must_use]
    pub fn set(&self, index: usize, value: &Var) -> Var {
        let mut new_vec = self.inner.clone();
        new_vec[index] = value.clone();
        Variant::List(Self::from_imvec(new_vec)).into()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Var> {
        self.inner.iter()
    }

    pub fn to_vec(&self) -> Vec<Var> {
        self.inner.clone().into_iter().collect()
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<usize> for List {
    type Output = Var;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

impl Display for List {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{{")?;
        let mut first = true;
        for v in self.inner.iter() {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            write!(f, "{v}")?;
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use crate::var::list::List;
    use crate::var::{v_int, v_list, v_string};

    #[test]
    pub fn weird_moo_insert_scenarios() {
        // MOO supports negative indexes, which just floor to 0...
        let list = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        assert_eq!(
            list.insert(-1, &v_int(0)),
            v_list(&[v_int(0), v_int(1), v_int(2), v_int(3)])
        );

        // MOO supports indexes beyond length of the list, which just append to the end...
        let list = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        assert_eq!(
            list.insert(100, &v_int(0)),
            v_list(&[v_int(1), v_int(2), v_int(3), v_int(0)])
        );
    }

    #[test]
    pub fn list_display() {
        let list = List::from_slice(&[v_int(1), v_string("foo".into()), v_int(3)]);
        assert_eq!(format!("{list}"), "{1, \"foo\", 3}");
    }
}
