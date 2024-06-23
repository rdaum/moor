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

use std::fmt::{Display, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};

use bincode::{Decode, Encode};
use bytes::Bytes;

#[allow(unused_imports)]
use crate::var::list_impl_buffer::ListImplBuffer;
#[allow(unused_imports)]
use crate::var::list_impl_vector::ListImplVector;

use crate::var::variant::Variant;
use crate::var::Var;
use crate::{AsByteBuffer, DecodingError, EncodingError};

#[cfg(feature = "list_impl_buffer")]
type ListImpl = ListImplBuffer;

#[cfg(not(feature = "list_impl_buffer"))]
type ListImpl = ListImplVector;

#[derive(Clone, Debug, Encode, Decode)]
pub struct List(ListImpl);

impl List {
    pub fn new() -> Self {
        Self(ListImpl::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<Var> {
        self.0.get(index)
    }

    pub fn from_slice(vec: &[Var]) -> List {
        Self(ListImpl::from_slice(vec))
    }

    // expensive because we need to extend both buffer length and the offsets length...
    pub fn push(&mut self, v: Var) -> Var {
        Var::new(Variant::List(Self(self.0.push(v))))
    }

    pub fn pop_front(&self) -> (Var, Var) {
        let results = self.0.pop_front();
        (results.0, Var::new(Variant::List(Self(results.1))))
    }

    pub fn append(&mut self, other: &Self) -> Var {
        Var::new(Variant::List(Self(self.0.append(other.0.clone()))))
    }

    pub fn remove_at(&mut self, index: usize) -> Var {
        Var::new(Variant::List(Self(self.0.remove_at(index))))
    }

    /// Remove the first found instance of the given value from the list.
    #[must_use]
    pub fn setremove(&mut self, value: &Var) -> Var {
        Var::new(Variant::List(Self(self.0.setremove(value))))
    }

    pub fn insert(&mut self, index: isize, value: Var) -> Var {
        Var::new(Variant::List(Self(self.0.insert(index, value))))
    }

    pub fn set(&mut self, index: usize, value: Var) -> Var {
        Var::new(Variant::List(Self(self.0.set(index, value))))
    }

    // Case insensitive
    pub fn contains(&self, v: &Var) -> bool {
        self.iter().any(|item| item.eq(v))
    }

    pub fn iter(&self) -> impl Iterator<Item = Var> + '_ {
        (0..self.len()).map(move |i| self.get(i).unwrap())
    }

    pub fn contains_case_sensitive(&self, v: &Var) -> bool {
        if let Variant::Str(s) = v.variant() {
            for item in self.iter() {
                if let Variant::Str(s2) = item.variant() {
                    if s.as_str() == s2.as_str() {
                        return true;
                    }
                }
            }
            return false;
        }
        self.contains(v)
    }
}

impl From<List> for Vec<Var> {
    fn from(value: List) -> Self {
        let len = value.len();
        let mut result = Vec::with_capacity(len);
        for i in 0..len {
            result.push(value.get(i).unwrap());
        }
        result
    }
}

impl AsByteBuffer for List {
    fn size_bytes(&self) -> usize {
        self.0.size_bytes()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        self.0.with_byte_buffer(|buf| f(buf))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        self.0.make_copy_as_vec()
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        Ok(Self(ListImpl::from_bytes(bytes)?))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        self.0.as_bytes()
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (a, b) in self.iter().zip(other.iter()) {
            if !a.eq(&b) {
                return false;
            }
        }
        true
    }
}
impl Eq for List {}

impl Hash for List {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for item in self.iter() {
            item.hash(state);
        }
    }
}
impl PartialOrd for List {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for List {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let len = self.len();
        if len != other.len() {
            return len.cmp(&other.len());
        }

        for (a, b) in self.iter().zip(other.iter()) {
            match a.cmp(&b) {
                std::cmp::Ordering::Equal => continue,
                x => return x,
            }
        }
        std::cmp::Ordering::Equal
    }
}

impl From<Vec<Var>> for List {
    fn from(value: Vec<Var>) -> Self {
        Self::from_slice(&value)
    }
}

impl From<&[Var]> for List {
    fn from(value: &[Var]) -> Self {
        Self::from_slice(value)
    }
}

impl Display for List {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{{")?;
        let mut first = true;
        for v in self.iter() {
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
        let mut list = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        assert_eq!(
            list.insert(-1, v_int(0)),
            v_list(&[v_int(0), v_int(1), v_int(2), v_int(3)])
        );

        // MOO supports indexes beyond length of the list, which just append to the end...
        let mut list = List::from_slice(&[v_int(1), v_int(2), v_int(3)]);
        assert_eq!(
            list.insert(100, v_int(0)),
            v_list(&[v_int(1), v_int(2), v_int(3), v_int(0)])
        );
    }

    #[test]
    pub fn list_display() {
        let list = List::from_slice(&[v_int(1), v_string("foo".into()), v_int(3)]);
        assert_eq!(format!("{list}"), "{1, \"foo\", 3}");
    }
}
