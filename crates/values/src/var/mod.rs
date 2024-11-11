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

mod error;
mod list;
mod map;
mod objid;
mod scalar;
mod string;
mod symbol;
#[allow(clippy::module_inception)]
mod var;
mod variant;

pub use error::{Error, ErrorPack};
pub use list::List;
pub use map::Map;
pub use objid::Objid;
use std::fmt::Debug;
pub use string::Str;
use strum::FromRepr;
pub use symbol::Symbol;
pub use var::{
    v_bool, v_empty_list, v_empty_map, v_empty_str, v_err, v_float, v_int, v_list, v_list_iter,
    v_map, v_none, v_obj, v_objid, v_str, v_string, Var,
};
pub use variant::Variant;

/// Integer encoding of values as represented in a `LambdaMOO` textdump, and by `bf_typeof`
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, FromRepr)]
#[allow(non_camel_case_types)]
pub enum VarType {
    TYPE_INT = 0,
    TYPE_OBJ = 1,
    TYPE_STR = 2,
    TYPE_ERR = 3,
    TYPE_LIST = 4,
    TYPE_NONE = 6,  // in uninitialized MOO variables */
    TYPE_LABEL = 7, // present only in textdump */
    TYPE_FLOAT = 9,
    TYPE_MAP = 10,
}

/// Sequence index modes: 0 or 1 indexed.
/// This is used to determine how to handle index operations on sequences. Internally containers use
/// 0-based indexing, but MOO uses 1-based indexing, so we allow the user to choose.
#[derive(Clone, Copy, Debug)]
pub enum IndexMode {
    ZeroBased,
    OneBased,
}

impl IndexMode {
    pub fn adjust_i64(&self, index: i64) -> isize {
        match self {
            IndexMode::ZeroBased => index as isize,
            IndexMode::OneBased => (index - 1) as isize,
        }
    }

    pub fn adjust_isize(&self, index: isize) -> isize {
        match self {
            IndexMode::ZeroBased => index,
            IndexMode::OneBased => index - 1,
        }
    }

    pub fn reverse_adjust_isize(&self, index: isize) -> isize {
        match self {
            IndexMode::ZeroBased => index,
            IndexMode::OneBased => index + 1,
        }
    }

    pub fn reverse_adjust_i64(&self, index: i64) -> isize {
        match self {
            IndexMode::ZeroBased => index as isize,
            IndexMode::OneBased => (index + 1) as isize,
        }
    }
}

pub enum TypeClass<'a> {
    Sequence(&'a dyn Sequence),
    Associative(&'a dyn Associative),
    Scalar,
}

impl<'a> TypeClass<'a> {
    fn is_sequence(&self) -> bool {
        matches!(self, TypeClass::Sequence(_))
    }

    fn is_associative(&self) -> bool {
        matches!(self, TypeClass::Associative(_))
    }

    fn is_scalar(&self) -> bool {
        matches!(self, TypeClass::Scalar)
    }
}

pub trait Sequence {
    /// Return true if the sequence is empty.
    fn is_empty(&self) -> bool;
    /// Return the length of the sequence.
    fn len(&self) -> usize;
    /// Check if the sequence contains the element, returning its offset if it does.
    /// `case_sensitive` is used to determine if the comparison should be case-sensitive.
    /// (MOO case sensitivity is often false)
    fn index_in(&self, value: &Var, case_sensitive: bool) -> Result<Option<usize>, Error>;
    /// Check if the sequence contains the element, returning true if it does.
    fn contains(&self, value: &Var, case_sensitive: bool) -> Result<bool, Error>;
    /// Get the `index`nth element of the sequence.
    fn index(&self, index: usize) -> Result<Var, Error>;
    /// Assign a new value to `index`nth element of the sequence.
    fn index_set(&self, index: usize, value: &Var) -> Result<Var, Error>;
    // Take a copy, add a new value to the end, and return it.
    fn push(&self, value: &Var) -> Result<Var, Error>;
    /// Insert a new value at `index` in the sequence.
    fn insert(&self, index: usize, value: &Var) -> Result<Var, Error>;
    /// Return a sequence which is a subset of this sequence where the indices lay between `from`
    /// and `to`, inclusive.
    fn range(&self, from: isize, to: isize) -> Result<Var, Error>;
    /// Assign new values to the sequence where the indices lay between `from` and `to`, inclusive.
    fn range_set(&self, from: isize, to: isize, with: &Var) -> Result<Var, Error>;
    /// Append the given sequence to this sequence.
    fn append(&self, other: &Var) -> Result<Var, Error>;
    /// Remove the `index`nth element of the sequence and return it.
    fn remove_at(&self, index: usize) -> Result<Var, Error>;
}

pub trait Associative {
    /// Return true if the associative container is empty.
    fn is_empty(&self) -> bool;
    /// Return the number of key-value pairs in the associative container.
    fn len(&self) -> usize;
    /// Get the value associated with the given key.
    fn index(&self, key: &Var) -> Result<Var, Error>;
    /// Find the position of the key in the associative container, that is, the offset of the key in
    /// the list of keys.
    /// `case_sensitive` is used to determine if the comparison should be case-sensitive.
    /// (MOO case sensitivity is often false)
    fn index_in(&self, key: &Var, case_sensitive: bool) -> Result<Option<usize>, Error>;
    /// Assign a new value to the given key.
    fn index_set(&self, key: &Var, value: &Var) -> Result<Var, Error>;
    /// Return the key-value pairs in the associative container between the given `from` and `to`
    fn range(&self, from: &Var, to: &Var) -> Result<Var, Error>;
    /// Assign new values to the key-value pairs in the associative container between the given `from` and `to`
    fn range_set(&self, from: &Var, to: &Var, with: &Var) -> Result<Var, Error>;
    /// Return the keys in the associative container.
    fn keys(&self) -> Vec<Var>;
    /// Return the values in the associative container.
    fn values(&self) -> Vec<Var>;
    /// Check if the associative container contains the key, returning true if it does.
    fn contains_key(&self, key: &Var, case_sensitive: bool) -> Result<bool, Error>;
    /// Return this map with the key/value pair removed.
    /// Return the new map and the value that was removed, if any
    fn remove(&self, key: &Var, case_sensitive: bool) -> (Var, Option<Var>);
}
