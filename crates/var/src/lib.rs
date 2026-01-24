// Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
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

mod binary;
pub mod encode;
mod error;
mod flyweight;
mod lambda;
mod list;
mod map;
mod obj;
pub mod program;
mod scalar;
mod string;
mod symbol;
mod variant;

pub use binary::Binary;
pub use error::{Error, ErrorCode, ErrorCode::*};
pub use flyweight::Flyweight;
pub use lambda::Lambda;
pub use list::List;
pub use map::Map;
pub use obj::{AMBIGUOUS, AnonymousObjid, FAILED_MATCH, NOTHING, Obj, SYSTEM_OBJECT, UuObjid};
use std::fmt::Debug;
pub use string::Str;
use strum::FromRepr;
pub use symbol::Symbol;
pub use variant::{
    OP_HINT_FLYWEIGHT_ADD_SLOT, OP_HINT_FLYWEIGHT_APPEND_CONTENTS, OP_HINT_LIST_APPEND,
    OP_HINT_MAP_INSERT, OP_HINT_NONE, OP_HINT_STR_APPEND, Var, Variant, v_arc_str, v_binary,
    v_bool, v_bool_int, v_empty_list, v_empty_map, v_empty_str, v_err, v_error, v_float,
    v_flyweight, v_int, v_list, v_list_iter, v_map, v_map_iter, v_none, v_nothing, v_obj, v_objid,
    v_str, v_string, v_sym,
};

pub use encode::{ByteSized, DecodingError, EncodingError};

/// Integer encoding of common as represented in a `LambdaMOO` textdump, and by `bf_typeof`
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, FromRepr)]
#[allow(non_camel_case_types)]
pub enum VarType {
    TYPE_INT = 0,
    TYPE_OBJ = 1,
    TYPE_STR = 2,
    TYPE_ERR = 3,
    TYPE_LIST = 4,
    _TYPE_CLEAR = 5,
    TYPE_NONE = 6,  // in uninitialized MOO variables */
    TYPE_LABEL = 7, // present only in textdump as TYPE_CATCH but it's a label*/
    _TYPE_FINALLY = 8,
    TYPE_FLOAT = 9,
    TYPE_MAP = 10,
    _TOAST_TYPE_ITER = 11,
    _TOAST_TYPE_ANON = 12,
    _TOAST_TYPE_WAIF = 13,
    TYPE_BOOL = 14,
    TYPE_FLYWEIGHT = 15,
    TYPE_SYMBOL = 16,
    TYPE_BINARY = 17,
    TYPE_LAMBDA = 18,
}

impl VarType {
    /// Convert to the canonical MOO source literal form (TYPE_INT, TYPE_OBJ, etc.)
    pub fn to_literal(&self) -> &str {
        match self {
            VarType::TYPE_INT => "TYPE_INT",
            VarType::TYPE_OBJ => "TYPE_OBJ",
            VarType::TYPE_FLOAT => "TYPE_FLOAT",
            VarType::TYPE_STR => "TYPE_STR",
            VarType::TYPE_ERR => "TYPE_ERR",
            VarType::TYPE_LIST => "TYPE_LIST",
            VarType::TYPE_MAP => "TYPE_MAP",
            VarType::TYPE_BOOL => "TYPE_BOOL",
            VarType::TYPE_FLYWEIGHT => "TYPE_FLYWEIGHT",
            VarType::TYPE_SYMBOL => "TYPE_SYM",
            VarType::TYPE_BINARY => "TYPE_BINARY",
            VarType::TYPE_LAMBDA => "TYPE_LAMBDA",
            _ => "INVALID-TYPE",
        }
    }

    /// Parse type constant from source. Accepts both new (TYPE_*) and legacy (*) forms.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            // New canonical forms
            "TYPE_NUM" | "TYPE_INT" => Some(VarType::TYPE_INT),
            "TYPE_FLOAT" => Some(VarType::TYPE_FLOAT),
            "TYPE_OBJ" => Some(VarType::TYPE_OBJ),
            "TYPE_STR" => Some(VarType::TYPE_STR),
            "TYPE_ERR" => Some(VarType::TYPE_ERR),
            "TYPE_LIST" => Some(VarType::TYPE_LIST),
            "TYPE_MAP" => Some(VarType::TYPE_MAP),
            "TYPE_BOOL" => Some(VarType::TYPE_BOOL),
            "TYPE_FLYWEIGHT" => Some(VarType::TYPE_FLYWEIGHT),
            "TYPE_SYM" => Some(VarType::TYPE_SYMBOL),
            "TYPE_BINARY" => Some(VarType::TYPE_BINARY),
            "TYPE_LAMBDA" => Some(VarType::TYPE_LAMBDA),
            // Legacy forms (for backwards compatibility)
            "NUM" | "INT" => Some(VarType::TYPE_INT),
            "FLOAT" => Some(VarType::TYPE_FLOAT),
            "OBJ" => Some(VarType::TYPE_OBJ),
            "STR" => Some(VarType::TYPE_STR),
            "ERR" => Some(VarType::TYPE_ERR),
            "LIST" => Some(VarType::TYPE_LIST),
            "MAP" => Some(VarType::TYPE_MAP),
            "BOOL" => Some(VarType::TYPE_BOOL),
            "FLYWEIGHT" => Some(VarType::TYPE_FLYWEIGHT),
            "SYM" => Some(VarType::TYPE_SYMBOL),
            "BINARY" => Some(VarType::TYPE_BINARY),
            "LAMBDA" => Some(VarType::TYPE_LAMBDA),
            _ => None,
        }
    }

    /// Parse only legacy (short) type constant names (INT, OBJ, STR, etc.)
    /// Returns None for new-style TYPE_* names.
    /// Used by the parser when legacy_type_constants mode is enabled.
    pub fn parse_legacy(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "NUM" | "INT" => Some(VarType::TYPE_INT),
            "FLOAT" => Some(VarType::TYPE_FLOAT),
            "OBJ" => Some(VarType::TYPE_OBJ),
            "STR" => Some(VarType::TYPE_STR),
            "ERR" => Some(VarType::TYPE_ERR),
            "LIST" => Some(VarType::TYPE_LIST),
            "MAP" => Some(VarType::TYPE_MAP),
            "BOOL" => Some(VarType::TYPE_BOOL),
            "FLYWEIGHT" => Some(VarType::TYPE_FLYWEIGHT),
            "SYM" => Some(VarType::TYPE_SYMBOL),
            "BINARY" => Some(VarType::TYPE_BINARY),
            "LAMBDA" => Some(VarType::TYPE_LAMBDA),
            _ => None,
        }
    }
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

impl TypeClass<'_> {
    pub fn is_scalar(&self) -> bool {
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
    /// Assign new common to the sequence where the indices lay between `from` and `to`, inclusive.
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
    /// Find the position of the key in the associative container, that is, the offset of the key in
    /// the list of keys.
    /// `case_sensitive` is used to determine if the comparison should be case-sensitive.
    /// (MOO case sensitivity is often false)
    fn index_in(&self, key: &Var, case_sensitive: bool) -> Result<Option<usize>, Error>;
    /// Get the key-value pair associated with the given key.
    fn get(&self, key: &Var) -> Result<Var, Error>;
    /// Update the key-value pair associated with the given key.
    fn set(&self, key: &Var, value: &Var) -> Result<Var, Error>;
    /// Get the `index`nth element of the sequence.
    fn index(&self, index: usize) -> Result<(Var, Var), Error>;
    /// Return the key-value pairs in the associative container between the given `from` and `to`
    fn range(&self, from: &Var, to: &Var) -> Result<Var, Error>;
    /// Assign new common to the key-value pairs in the associative container between the given `from` and `to`
    fn range_set(&self, from: &Var, to: &Var, with: &Var) -> Result<Var, Error>;
    /// Return the keys in the associative container.
    fn keys(&self) -> Vec<Var>;
    /// Return the common in the associative container.
    fn values(&self) -> Vec<Var>;
    /// Check if the associative container contains the key, returning true if it does.
    fn contains_key(&self, key: &Var, case_sensitive: bool) -> Result<bool, Error>;
    /// Return this map with the key/value pair removed.
    /// Return the new map and the value that was removed, if any
    fn remove(&self, key: &Var, case_sensitive: bool) -> (Var, Option<Var>);
    /// Get the first key/value pair in the association, or E_RANGE if empty
    fn first(&self) -> Result<(Var, Var), Error>;
    /// Get the next key after the given key in iteration order.
    /// Returns None if the key is the last one or doesn't exist.
    fn next_after(&self, key: &Var, case_sensitive: bool) -> Result<(Var, Var), Error>;
}
