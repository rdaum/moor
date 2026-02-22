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

//! MOO variable type with C-like representation optimized for fast clone operations.
//! Uses a type tag with COMPLEX_FLAG bit to enable single-branch clone for simple types.

use crate::{
    Associative, ByteSized, Error, Flyweight, IndexMode, NOTHING, Obj, Sequence, Symbol, TypeClass,
    VarType,
    binary::Binary,
    error::{
        ErrorCode,
        ErrorCode::{E_INVARG, E_RANGE, E_TYPE},
    },
    lambda::Lambda,
    list::List,
    map, string,
    string::Str,
};
use once_cell::sync::Lazy;
use std::{
    cmp::{Ordering, min},
    fmt::{Debug, Formatter},
    hash::{Hash, Hasher},
    sync::Arc,
};

// Type tags - simple types have low values, complex types have COMPLEX_FLAG set
const TAG_NONE: u8 = 0;
const TAG_BOOL_FALSE: u8 = 1;
const TAG_BOOL_TRUE: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_FLOAT: u8 = 4;
const TAG_OBJ: u8 = 5;
const TAG_SYM: u8 = 6;
const TAG_EMPTY_STR: u8 = 7;
const TAG_EMPTY_LIST: u8 = 8;
const TAG_SYMBOL_STR: u8 = 9;

const COMPLEX_FLAG: u8 = 0x80;
const TAG_STR: u8 = COMPLEX_FLAG | 1;
const TAG_LIST: u8 = COMPLEX_FLAG | 2;
const TAG_MAP: u8 = COMPLEX_FLAG | 3;
const TAG_ERR: u8 = COMPLEX_FLAG | 4;
const TAG_FLYWEIGHT: u8 = COMPLEX_FLAG | 5;
const TAG_BINARY: u8 = COMPLEX_FLAG | 6;
const TAG_LAMBDA: u8 = COMPLEX_FLAG | 7;

// Operation Hints (stored in meta[6])
// These provide hints to the conflict resolver about how this value was created.
pub const OP_HINT_NONE: u8 = 0;
pub const OP_HINT_LIST_APPEND: u8 = 1;
pub const OP_HINT_MAP_INSERT: u8 = 2;
pub const OP_HINT_FLYWEIGHT_ADD_SLOT: u8 = 3;
pub const OP_HINT_FLYWEIGHT_APPEND_CONTENTS: u8 = 4;
pub const OP_HINT_STR_APPEND: u8 = 5;

/// Borrowed empty string storage for TAG_EMPTY_STR accessors.
static EMPTY_STR: Lazy<Str> = Lazy::new(|| Str::mk_str(""));

/// Borrowed empty list storage for TAG_EMPTY_LIST accessors.
static EMPTY_LIST: Lazy<List> = Lazy::new(|| List::mk_list(&[]));

/// Cached empty string Var.
static EMPTY_STR_VAR: Lazy<Var> = Lazy::new(Var::mk_empty_str);

/// Cached empty list Var.
static EMPTY_LIST_VAR: Lazy<Var> = Lazy::new(Var::mk_empty_list);

/// Cached NOTHING object Var.
static NOTHING_VAR: Lazy<Var> = Lazy::new(|| Var::mk_object(NOTHING));

/// MOO variable - C-like representation optimized for fast clone.
/// 16 bytes total - tag + metadata + data pointer/value.
#[repr(C)]
pub struct Var {
    /// Type tag with COMPLEX_FLAG for refcounted types
    tag: u8,
    /// Metadata bytes - interpretation depends on tag (union semantics).
    /// For List/Map: bytes[0..2] = cached element count (u16 native-endian), byte[6] = op hint.
    /// For String: bytes[0..2] = cached char count, byte[2] = ASCII flag (1 if pure ASCII).
    /// For other types: unused (all zeros).
    meta: [u8; 7],
    /// Union of all possible values (inline for simple types, pointer for complex)
    data: u64,
}

/// Sentinel value indicating the cached length overflowed and real length must be checked.
const LEN_OVERFLOW: u16 = 0xFFFF;

impl Var {
    /// Get cached length from meta bytes (for List/Map/String).
    #[inline(always)]
    fn cached_len(&self) -> u16 {
        u16::from_ne_bytes([self.meta[0], self.meta[1]])
    }

    /// Get the operation hint from meta bytes.
    #[inline(always)]
    pub fn op_hint(&self) -> u8 {
        self.meta[6]
    }

    /// Return a copy of this Var with the operation hint cleared to OP_HINT_NONE.
    /// This should be called on merged values before committing to storage,
    /// as hints are only meaningful for the operation that created the value,
    /// not for the final committed state.
    #[inline(always)]
    pub fn with_cleared_hint(mut self) -> Self {
        self.meta[6] = OP_HINT_NONE;
        self
    }

    /// Create meta bytes with cached length.
    #[inline(always)]
    fn meta_with_len(len: usize) -> [u8; 7] {
        Self::meta_with_len_and_hint(len, OP_HINT_NONE)
    }

    /// Create meta bytes with cached length and operation hint.
    #[inline(always)]
    fn meta_with_len_and_hint(len: usize, hint: u8) -> [u8; 7] {
        let len16 = if len >= LEN_OVERFLOW as usize {
            LEN_OVERFLOW
        } else {
            len as u16
        };
        let bytes = len16.to_ne_bytes();
        [bytes[0], bytes[1], 0, 0, 0, 0, hint]
    }

    /// Create meta bytes with cached length, ASCII flag, and hint for strings.
    #[inline(always)]
    fn meta_with_str_info(char_len: usize, byte_len: usize, hint: u8) -> [u8; 7] {
        let len16 = if char_len >= LEN_OVERFLOW as usize {
            LEN_OVERFLOW
        } else {
            char_len as u16
        };
        let bytes = len16.to_ne_bytes();
        let is_ascii = if byte_len == char_len { 1 } else { 0 };
        [bytes[0], bytes[1], is_ascii, 0, 0, 0, hint]
    }

    /// Check if a string Var contains only ASCII characters.
    /// Returns false for non-string types.
    #[inline(always)]
    pub fn str_is_ascii(&self) -> bool {
        self.tag == TAG_EMPTY_STR
            || self.tag == TAG_SYMBOL_STR
            || (self.tag == TAG_STR && self.meta[2] == 1)
    }

    // === String search and replace operations with cached ASCII optimization ===

    /// Find first occurrence of needle in self. Returns 0-based char index or None.
    /// Uses cached ASCII flag for fast path when both strings are ASCII.
    #[inline]
    pub fn str_find(&self, needle: &Var, case_matters: bool, skip: usize) -> Option<usize> {
        let subject: &string::Str = self.as_str()?;
        let needle_str: &string::Str = needle.as_str()?;
        let is_ascii = self.str_is_ascii() && needle.str_is_ascii();
        string::str_find(
            subject.as_str(),
            needle_str.as_str(),
            case_matters,
            skip,
            is_ascii,
        )
    }

    /// Find last occurrence of needle in self. Returns 0-based char index or None.
    /// Uses cached ASCII flag for fast path when both strings are ASCII.
    #[inline]
    pub fn str_rfind(
        &self,
        needle: &Var,
        case_matters: bool,
        skip_from_end: usize,
    ) -> Option<usize> {
        let subject: &string::Str = self.as_str()?;
        let needle_str: &string::Str = needle.as_str()?;
        let is_ascii = self.str_is_ascii() && needle.str_is_ascii();
        string::str_rfind(
            subject.as_str(),
            needle_str.as_str(),
            case_matters,
            skip_from_end,
            is_ascii,
        )
    }

    /// Replace all occurrences of `what` with `with` in self.
    /// Uses cached ASCII flag for fast path when all strings are ASCII.
    #[inline]
    pub fn str_replace(&self, what: &Var, with: &Var, case_matters: bool) -> Option<Var> {
        let subject: &string::Str = self.as_str()?;
        let what_str: &string::Str = what.as_str()?;
        let with_str: &string::Str = with.as_str()?;
        let is_ascii = self.str_is_ascii() && what.str_is_ascii() && with.str_is_ascii();
        let result = string::str_replace(
            subject.as_str(),
            what_str.as_str(),
            with_str.as_str(),
            case_matters,
            is_ascii,
        );
        Some(Var::mk_string(result))
    }
}

/// View type for pattern matching - constructed on demand from Var.
/// References point into the Var's data, so lifetime is tied to Var.
#[derive(Debug)]
pub enum Variant<'a> {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Obj(Obj),
    Sym(Symbol),
    Str(&'a string::Str),
    List(&'a List),
    Map(&'a map::Map),
    Err(&'a Error),
    Flyweight(&'a Flyweight),
    Binary(&'a Binary),
    Lambda(&'a Lambda),
}

impl Var {
    // === Constructors for simple types ===

    #[inline(always)]
    pub const fn mk_none() -> Self {
        Self {
            tag: TAG_NONE,
            meta: [0; 7],
            data: 0,
        }
    }

    #[inline(always)]
    pub const fn mk_bool(b: bool) -> Self {
        Self {
            tag: if b { TAG_BOOL_TRUE } else { TAG_BOOL_FALSE },
            meta: [0; 7],
            data: 0,
        }
    }

    #[inline(always)]
    pub const fn mk_integer(i: i64) -> Self {
        Self {
            tag: TAG_INT,
            meta: [0; 7],
            data: i as u64,
        }
    }

    #[inline(always)]
    pub fn mk_float(f: f64) -> Self {
        Self {
            tag: TAG_FLOAT,
            meta: [0; 7],
            data: f.to_bits(),
        }
    }

    #[inline(always)]
    pub fn mk_object(o: Obj) -> Self {
        Self {
            tag: TAG_OBJ,
            meta: [0; 7],
            data: o.as_u64(),
        }
    }

    #[inline(always)]
    pub fn mk_symbol(s: Symbol) -> Self {
        // SAFETY: Symbol is repr(C) with two u32s = 8 bytes = u64
        let data: u64 = unsafe { std::mem::transmute(s) };
        Self {
            tag: TAG_SYM,
            meta: [0; 7],
            data,
        }
    }

    // === Constructors for complex types ===
    // Str, List, Map, Lambda are #[repr(transparent)] Arc wrappers (8 bytes)
    // We store them directly via transmute - no Box needed!

    pub fn mk_str(s: &str) -> Self {
        if s.is_empty() {
            return Self::mk_empty_str();
        }
        Self::from_str_type(Str::mk_str(s))
    }

    pub fn mk_string(s: String) -> Self {
        if s.is_empty() {
            return Self::mk_empty_str();
        }
        Self::from_str_type(Str::from(s))
    }

    #[inline(always)]
    pub const fn mk_empty_str() -> Self {
        Self::mk_empty_str_with_hint(OP_HINT_NONE)
    }

    #[inline(always)]
    pub const fn mk_empty_str_with_hint(hint: u8) -> Self {
        Self {
            tag: TAG_EMPTY_STR,
            meta: [0, 0, 1, 0, 0, 0, hint],
            data: 0,
        }
    }

    /// Create a Var from a Str type directly
    pub fn from_str_type(s: string::Str) -> Self {
        Self::from_str_type_with_hint(s, OP_HINT_NONE)
    }

    /// Create a string-typed Var from an interned symbol.
    /// This avoids ArcStr clone/drop traffic on Var clone/drop.
    pub fn from_symbol_str(symbol: Symbol) -> Self {
        Self::from_symbol_str_with_hint(symbol, OP_HINT_NONE)
    }

    /// Create a symbol-backed string Var with an operation hint.
    pub fn from_symbol_str_with_hint(symbol: Symbol, hint: u8) -> Self {
        let str_ref = symbol.as_str();
        if str_ref.is_empty() {
            return Self::mk_empty_str_with_hint(hint);
        }
        let byte_len = str_ref.len();
        let char_len = str_ref.chars().count();
        // SAFETY: Symbol is repr(C) with two u32 fields, exactly 8 bytes.
        let data: u64 = unsafe { std::mem::transmute(symbol) };
        Self {
            tag: TAG_SYMBOL_STR,
            meta: Self::meta_with_str_info(char_len, byte_len, hint),
            data,
        }
    }

    /// Create a Var from a Str type with an operation hint
    pub fn from_str_type_with_hint(s: string::Str, hint: u8) -> Self {
        let str_ref = s.as_str();
        if str_ref.is_empty() {
            return Self::mk_empty_str_with_hint(hint);
        }
        let byte_len = str_ref.len();
        let char_len = str_ref.chars().count();
        // SAFETY: Str is #[repr(transparent)] around Arc<String>, exactly 8 bytes
        let data: u64 = unsafe { std::mem::transmute(s) };
        Self {
            tag: TAG_STR,
            meta: Self::meta_with_str_info(char_len, byte_len, hint),
            data,
        }
    }

    pub fn mk_list(values: &[Var]) -> Self {
        if values.is_empty() {
            return Self::mk_empty_list();
        }
        List::build(values)
    }

    pub fn mk_list_iter<IT: IntoIterator<Item = Var>>(values: IT) -> Self {
        Var::from_iter(values)
    }

    /// Create a Var from a List directly
    pub fn from_list(list: List) -> Self {
        Self::from_list_with_hint(list, OP_HINT_NONE)
    }

    /// Create a Var from a List with an operation hint
    pub fn from_list_with_hint(list: List, hint: u8) -> Self {
        let len = list.len();
        if len == 0 {
            return Self::mk_empty_list_with_hint(hint);
        }
        // SAFETY: List is #[repr(transparent)] around Box<Vector>, exactly 8 bytes
        let data: u64 = unsafe { std::mem::transmute(list) };
        Self {
            tag: TAG_LIST,
            meta: Self::meta_with_len_and_hint(len, hint),
            data,
        }
    }

    #[inline(always)]
    pub const fn mk_empty_list() -> Self {
        Self::mk_empty_list_with_hint(OP_HINT_NONE)
    }

    #[inline(always)]
    pub const fn mk_empty_list_with_hint(hint: u8) -> Self {
        Self {
            tag: TAG_EMPTY_LIST,
            meta: [0, 0, 0, 0, 0, 0, hint],
            data: 0,
        }
    }

    pub fn mk_map(pairs: &[(Var, Var)]) -> Self {
        map::Map::build(pairs.iter())
    }

    pub fn mk_map_iter<'a, I: Iterator<Item = &'a (Var, Var)>>(pairs: I) -> Self {
        map::Map::build(pairs)
    }

    /// Create a Var from a Map directly
    pub fn from_map(m: map::Map) -> Self {
        Self::from_map_with_hint(m, OP_HINT_NONE)
    }

    /// Create a Var from a Map with an operation hint
    pub fn from_map_with_hint(m: map::Map, hint: u8) -> Self {
        let len = m.len();
        // SAFETY: Map is #[repr(transparent)] around Box<OrdMap>, exactly 8 bytes
        let data: u64 = unsafe { std::mem::transmute(m) };
        Self {
            tag: TAG_MAP,
            meta: Self::meta_with_len_and_hint(len, hint),
            data,
        }
    }

    pub fn mk_error(e: Error) -> Self {
        let arced = Arc::new(e);
        Self {
            tag: TAG_ERR,
            meta: [0; 7],
            data: Arc::into_raw(arced) as u64,
        }
    }

    pub fn mk_error_arc(e: Arc<Error>) -> Self {
        Self {
            tag: TAG_ERR,
            meta: [0; 7],
            data: Arc::into_raw(e) as u64,
        }
    }

    pub fn mk_binary(bytes: Vec<u8>) -> Self {
        let len = bytes.len();
        let boxed = Box::new(Binary::from_bytes(bytes));
        Self {
            tag: TAG_BINARY,
            meta: Self::meta_with_len(len),
            data: Box::into_raw(boxed) as u64,
        }
    }

    /// Create a Var from a Flyweight directly
    pub fn from_flyweight(f: Flyweight) -> Self {
        Self::from_flyweight_with_hint(f, OP_HINT_NONE)
    }

    /// Create a Var from a Flyweight with an operation hint
    pub fn from_flyweight_with_hint(f: Flyweight, hint: u8) -> Self {
        let boxed = Box::new(f);
        Self {
            tag: TAG_FLYWEIGHT,
            meta: [0, 0, 0, 0, 0, 0, hint],
            data: Box::into_raw(boxed) as u64,
        }
    }

    pub fn mk_lambda(
        params: crate::program::opcode::ScatterArgs,
        body: crate::program::program::Program,
        captured_env: Vec<Vec<Var>>,
        self_var: Option<crate::program::names::Name>,
    ) -> Self {
        let lambda = Lambda::new(params, body, captured_env, self_var);
        // SAFETY: Lambda is #[repr(transparent)] around Arc<LambdaInner>, exactly 8 bytes
        let data: u64 = unsafe { std::mem::transmute(lambda) };
        Self {
            tag: TAG_LAMBDA,
            meta: [0; 7],
            data,
        }
    }

    /// Create from Lambda directly
    pub fn from_lambda(l: Lambda) -> Self {
        // SAFETY: Lambda is #[repr(transparent)] around Arc<LambdaInner>, exactly 8 bytes
        let data: u64 = unsafe { std::mem::transmute(l) };
        Self {
            tag: TAG_LAMBDA,
            meta: [0; 7],
            data,
        }
    }

    // === View for pattern matching ===

    #[inline]
    pub fn variant(&self) -> Variant<'_> {
        match self.tag {
            TAG_NONE => Variant::None,
            TAG_BOOL_FALSE => Variant::Bool(false),
            TAG_BOOL_TRUE => Variant::Bool(true),
            TAG_INT => Variant::Int(self.data as i64),
            TAG_FLOAT => Variant::Float(f64::from_bits(self.data)),
            TAG_OBJ => {
                let obj: Obj = unsafe { std::mem::transmute(self.data) };
                Variant::Obj(obj)
            }
            TAG_SYM => {
                let sym: Symbol = unsafe { std::mem::transmute(self.data) };
                Variant::Sym(sym)
            }
            TAG_EMPTY_STR => Variant::Str(&EMPTY_STR),
            TAG_EMPTY_LIST => Variant::List(&EMPTY_LIST),
            TAG_SYMBOL_STR => Variant::Str(self.as_str().unwrap()),
            // Str, List, Map, Lambda: data contains transmuted value, reinterpret &data as &Type
            TAG_STR => Variant::Str(unsafe { &*(&self.data as *const u64 as *const string::Str) }),
            TAG_LIST => Variant::List(unsafe { &*(&self.data as *const u64 as *const List) }),
            TAG_MAP => Variant::Map(unsafe { &*(&self.data as *const u64 as *const map::Map) }),
            TAG_LAMBDA => Variant::Lambda(unsafe { &*(&self.data as *const u64 as *const Lambda) }),
            // Err, Flyweight, Binary: data is a pointer (Arc or Box)
            TAG_ERR => Variant::Err(unsafe { &*(self.data as *const Error) }),
            TAG_FLYWEIGHT => Variant::Flyweight(unsafe { &*(self.data as *const Flyweight) }),
            TAG_BINARY => Variant::Binary(unsafe { &*(self.data as *const Binary) }),
            _ => unreachable!("invalid tag"),
        }
    }

    #[inline(always)]
    fn is_simple(&self) -> bool {
        self.tag & COMPLEX_FLAG == 0
    }

    // === Direct accessor methods ===

    #[inline(always)]
    pub fn as_integer(&self) -> Option<i64> {
        if self.tag == TAG_INT {
            Some(self.data as i64)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn as_float(&self) -> Option<f64> {
        if self.tag == TAG_FLOAT {
            Some(f64::from_bits(self.data))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn as_bool(&self) -> Option<bool> {
        match self.tag {
            TAG_BOOL_TRUE => Some(true),
            TAG_BOOL_FALSE => Some(false),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn as_object(&self) -> Option<Obj> {
        if self.tag == TAG_OBJ {
            Some(unsafe { std::mem::transmute::<u64, Obj>(self.data) })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn as_sym(&self) -> Option<Symbol> {
        if self.tag == TAG_SYM {
            Some(unsafe { std::mem::transmute::<u64, Symbol>(self.data) })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn is_none(&self) -> bool {
        self.tag == TAG_NONE
    }

    #[inline(always)]
    pub fn is_int(&self) -> bool {
        self.tag == TAG_INT
    }

    #[inline(always)]
    pub fn is_float(&self) -> bool {
        self.tag == TAG_FLOAT
    }

    #[inline(always)]
    pub fn is_obj(&self) -> bool {
        self.tag == TAG_OBJ
    }

    #[inline(always)]
    pub fn is_list(&self) -> bool {
        self.tag == TAG_LIST || self.tag == TAG_EMPTY_LIST
    }

    /// Check if this is a numeric zero (int 0 or float 0.0).
    #[inline(always)]
    pub fn is_zero(&self) -> bool {
        match self.tag {
            TAG_INT => self.data == 0,
            TAG_FLOAT => f64::from_bits(self.data) == 0.0,
            _ => false,
        }
    }

    /// Check if both self and other are integers.
    #[inline(always)]
    pub fn both_int(&self, other: &Self) -> bool {
        self.tag == TAG_INT && other.tag == TAG_INT
    }

    /// Check if both self and other are floats.
    #[inline(always)]
    pub fn both_float(&self, other: &Self) -> bool {
        self.tag == TAG_FLOAT && other.tag == TAG_FLOAT
    }

    /// Check if both self and other are objects.
    #[inline(always)]
    pub fn both_obj(&self, other: &Self) -> bool {
        self.tag == TAG_OBJ && other.tag == TAG_OBJ
    }

    /// Check if both are the same simple numeric type (int, float, or obj).
    #[inline(always)]
    pub fn same_numeric_type(&self, other: &Self) -> bool {
        self.tag == other.tag && matches!(self.tag, TAG_INT | TAG_FLOAT | TAG_OBJ)
    }

    // Str, List, Map, Lambda: data contains transmuted value
    #[inline(always)]
    pub fn as_str(&self) -> Option<&string::Str> {
        match self.tag {
            TAG_STR => Some(unsafe { &*(&self.data as *const u64 as *const string::Str) }),
            TAG_EMPTY_STR => Some(&*EMPTY_STR),
            TAG_SYMBOL_STR => {
                let sym: Symbol = unsafe { std::mem::transmute(self.data) };
                let arc_ref = sym.as_arc_str_ref();
                // SAFETY: Str is repr(transparent) over ArcStr.
                Some(unsafe { &*(arc_ref as *const arcstr::ArcStr as *const string::Str) })
            }
            _ => None,
        }
    }

    /// Extract the string value if this is a string, otherwise None.
    #[inline]
    pub fn as_string(&self) -> Option<&str> {
        self.as_str().map(|s: &string::Str| s.as_str())
    }

    #[inline(always)]
    pub fn as_list(&self) -> Option<&List> {
        match self.tag {
            TAG_LIST => Some(unsafe { &*(&self.data as *const u64 as *const List) }),
            TAG_EMPTY_LIST => Some(&*EMPTY_LIST),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn as_map(&self) -> Option<&map::Map> {
        if self.tag == TAG_MAP {
            Some(unsafe { &*(&self.data as *const u64 as *const map::Map) })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn as_lambda(&self) -> Option<&Lambda> {
        if self.tag == TAG_LAMBDA {
            Some(unsafe { &*(&self.data as *const u64 as *const Lambda) })
        } else {
            None
        }
    }

    // Err, Flyweight, Binary: data is a pointer
    #[inline(always)]
    pub fn as_error(&self) -> Option<&Error> {
        if self.tag == TAG_ERR {
            Some(unsafe { &*(self.data as *const Error) })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn as_flyweight(&self) -> Option<&Flyweight> {
        if self.tag == TAG_FLYWEIGHT {
            Some(unsafe { &*(self.data as *const Flyweight) })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn as_binary(&self) -> Option<&Binary> {
        if self.tag == TAG_BINARY {
            Some(unsafe { &*(self.data as *const Binary) })
        } else {
            None
        }
    }

    // === Type information ===

    pub fn type_code(&self) -> VarType {
        // Direct tag lookup avoids constructing Variant enum
        match self.tag {
            TAG_NONE => VarType::TYPE_NONE,
            TAG_BOOL_FALSE | TAG_BOOL_TRUE => VarType::TYPE_BOOL,
            TAG_INT => VarType::TYPE_INT,
            TAG_FLOAT => VarType::TYPE_FLOAT,
            TAG_OBJ => VarType::TYPE_OBJ,
            TAG_SYM => VarType::TYPE_SYMBOL,
            TAG_STR | TAG_EMPTY_STR | TAG_SYMBOL_STR => VarType::TYPE_STR,
            TAG_LIST | TAG_EMPTY_LIST => VarType::TYPE_LIST,
            TAG_MAP => VarType::TYPE_MAP,
            TAG_ERR => VarType::TYPE_ERR,
            TAG_FLYWEIGHT => VarType::TYPE_FLYWEIGHT,
            TAG_BINARY => VarType::TYPE_BINARY,
            TAG_LAMBDA => VarType::TYPE_LAMBDA,
            _ => unreachable!("invalid tag"),
        }
    }

    /// If a string, turn into symbol, or if already a symbol, return that.
    /// Otherwise, E_TYPE
    pub fn as_symbol(&self) -> Result<Symbol, Error> {
        match self.variant() {
            Variant::Str(s) => Ok(Symbol::mk(s.as_str())),
            Variant::Sym(s) => Ok(s),
            Variant::Err(e) => Ok(e.name()),
            _ => Err(E_TYPE.with_msg(|| {
                format!("Cannot convert {} to symbol", self.type_code().to_literal())
            })),
        }
    }

    pub fn is_true(&self) -> bool {
        match self.tag {
            // Simple types - check directly without constructing Variant
            TAG_NONE | TAG_OBJ | TAG_ERR => false,
            TAG_BOOL_FALSE => false,
            TAG_BOOL_TRUE => true,
            TAG_INT => self.data != 0,
            TAG_FLOAT => f64::from_bits(self.data) != 0.0,
            TAG_SYM | TAG_LAMBDA => true,
            // Complex types - need to access the data
            TAG_EMPTY_STR => false,
            TAG_EMPTY_LIST => false,
            TAG_SYMBOL_STR => !self.as_str().unwrap().is_empty(),
            TAG_STR => !self.as_str().unwrap().is_empty(),
            TAG_LIST => !self.as_list().unwrap().is_empty(),
            TAG_MAP => !self.as_map().unwrap().is_empty(),
            TAG_FLYWEIGHT => !self.as_flyweight().unwrap().is_contents_empty(),
            TAG_BINARY => !self.as_binary().unwrap().is_empty(),
            _ => unreachable!("invalid tag"),
        }
    }

    pub fn type_class(&self) -> TypeClass<'_> {
        match self.tag {
            TAG_LIST | TAG_EMPTY_LIST => TypeClass::Sequence(self.as_list().unwrap()),
            TAG_STR | TAG_EMPTY_STR | TAG_SYMBOL_STR => TypeClass::Sequence(self.as_str().unwrap()),
            TAG_BINARY => TypeClass::Sequence(self.as_binary().unwrap()),
            TAG_MAP => TypeClass::Associative(self.as_map().unwrap()),
            _ => TypeClass::Scalar,
        }
    }

    pub fn is_sequence(&self) -> bool {
        matches!(
            self.tag,
            TAG_LIST | TAG_EMPTY_LIST | TAG_STR | TAG_EMPTY_STR | TAG_SYMBOL_STR | TAG_BINARY
        )
    }

    pub fn is_associative(&self) -> bool {
        self.tag == TAG_MAP
    }

    pub fn is_scalar(&self) -> bool {
        !self.is_sequence() && !self.is_associative()
    }

    pub fn is_string(&self) -> bool {
        self.tag == TAG_STR || self.tag == TAG_EMPTY_STR || self.tag == TAG_SYMBOL_STR
    }

    // === Collection operations ===

    /// Index into a sequence type, or get Nth element of an association set
    pub fn index(&self, index: &Var, index_mode: IndexMode) -> Result<Self, Error> {
        if self.is_scalar() {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot index into scalar value {}",
                    self.type_code().to_literal()
                )
            }));
        }

        let Some(i) = index.as_integer() else {
            return Err(E_TYPE.with_msg(|| {
                format!(
                    "Cannot index into sequence with non-integer index {}",
                    index.type_code().to_literal()
                )
            }));
        };

        let idx = {
            let i = index_mode.adjust_i64(i);
            if i < 0 {
                return Err(E_RANGE
                    .with_msg(|| format!("Cannot index into sequence with negative index {i}")));
            }
            i as usize
        };

        // Bounds check using cached length (avoids dereferencing on out-of-bounds)
        let cached = self.cached_len();
        if cached != LEN_OVERFLOW && idx >= cached as usize {
            return Err(E_RANGE.with_msg(|| {
                format!(
                    "Index {} out of bounds for {} of length {}",
                    idx + 1,
                    self.type_code().to_literal(),
                    cached
                )
            }));
        }

        // Dispatch directly on tag - we already know it's not scalar
        match self.tag {
            TAG_LIST | TAG_EMPTY_LIST => self.as_list().unwrap().index(idx),
            TAG_STR | TAG_EMPTY_STR | TAG_SYMBOL_STR => self.as_str().unwrap().index(idx),
            TAG_BINARY => self.as_binary().unwrap().index(idx),
            TAG_MAP => Ok(self.as_map().unwrap().index(idx)?.1),
            _ => unreachable!(),
        }
    }

    /// Return the associative key at `key`, or the Nth element of a sequence.
    pub fn get(&self, key: &Var, index_mode: IndexMode) -> Result<Self, Error> {
        match self.type_class() {
            TypeClass::Sequence(_) => self.index(key, index_mode),
            TypeClass::Associative(a) => a.get(key),
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
            TypeClass::Sequence(_) => self.index_set(key, value, index_mode),
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
        let Some(idx) = idx.as_integer() else {
            return Err(E_INVARG.with_msg(|| {
                format!(
                    "Cannot index into sequence with non-integer index {}",
                    idx.type_code().to_literal()
                )
            }));
        };
        let idx = {
            let i = index_mode.adjust_i64(idx);
            if i < 0 {
                return Err(E_RANGE
                    .with_msg(|| format!("Cannot index into sequence with negative index {i}")));
            }
            i as usize
        };
        let TypeClass::Sequence(s) = self.type_class() else {
            return Err(E_TYPE.with_msg(|| {
                format!("Cannot set value in type {}", self.type_code().to_literal())
            }));
        };
        s.index_set(idx, value)
    }

    /// Insert a new value at `index` in a sequence only.
    pub fn insert(&self, index: &Var, value: &Var, index_mode: IndexMode) -> Result<Var, Error> {
        match self.type_class() {
            TypeClass::Sequence(s) => {
                let index = match index.variant() {
                    Variant::Int(i) => index_mode.adjust_i64(i),
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
                    Variant::Int(i) => index_mode.adjust_i64(i),
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
                    Variant::Int(i) => index_mode.adjust_i64(i),
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
                    Variant::Int(i) => index_mode.adjust_i64(i),
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
                    Variant::Int(i) => index_mode.adjust_i64(i),
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
        // Fast path for strings: use str_find with cached ASCII flag
        if self.tag == TAG_STR || self.tag == TAG_EMPTY_STR || self.tag == TAG_SYMBOL_STR {
            if value.as_str().is_none() {
                return Err(E_TYPE.with_msg(|| {
                    format!(
                        "Cannot check if string contains {}",
                        value.type_code().to_literal()
                    )
                }));
            }
            let result = self.str_find(value, case_sensitive, 0).is_some();
            return Ok(v_bool_int(result));
        }

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
        // Fast path for strings: use str_find with cached ASCII flag
        if self.tag == TAG_STR || self.tag == TAG_EMPTY_STR || self.tag == TAG_SYMBOL_STR {
            if value.as_str().is_none() {
                return Err(E_TYPE.with_msg(|| {
                    format!(
                        "Cannot index string with {}",
                        value.type_code().to_literal()
                    )
                }));
            }
            let idx = self
                .str_find(value, case_sensitive, 0)
                .map(|i| i as i64)
                .unwrap_or(-1);
            return Ok(v_int(index_mode.reverse_adjust_isize(idx as isize) as i64));
        }

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
                    Variant::Int(i) => index_mode.adjust_i64(i),
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

    pub fn is_empty(&self) -> Result<bool, Error> {
        if self.is_scalar() {
            return Err(E_INVARG.with_msg(|| {
                format!(
                    "Cannot check if scalar value {} is empty",
                    self.type_code().to_literal()
                )
            }));
        }
        let cached = self.cached_len();
        if cached != LEN_OVERFLOW {
            return Ok(cached == 0);
        }
        // Overflow: fall back to real length check
        match self.type_class() {
            TypeClass::Sequence(s) => Ok(s.is_empty()),
            TypeClass::Associative(a) => Ok(a.is_empty()),
            TypeClass::Scalar => unreachable!(),
        }
    }

    pub fn len(&self) -> Result<usize, Error> {
        if self.is_scalar() {
            return Err(E_INVARG.with_msg(|| {
                format!(
                    "Cannot get length of scalar value {}",
                    self.type_code().to_literal()
                )
            }));
        }
        let cached = self.cached_len();
        if cached != LEN_OVERFLOW {
            return Ok(cached as usize);
        }
        // Overflow: fall back to real length
        match self.type_class() {
            TypeClass::Sequence(s) => Ok(s.len()),
            TypeClass::Associative(a) => Ok(a.len()),
            TypeClass::Scalar => unreachable!(),
        }
    }

    // === Comparison helpers ===

    pub fn eq_case_sensitive(&self, other: &Var) -> bool {
        match (self.variant(), other.variant()) {
            (Variant::Str(s1), Variant::Str(s2)) => s1.as_str() == s2.as_str(),
            (Variant::List(l1), Variant::List(l2)) => {
                if l1.len() != l2.len() {
                    return false;
                }
                for (left, right) in l1.iter().zip(l2.iter()) {
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
                for (left, right) in m1.iter().zip(m2.iter()) {
                    if !left.0.eq_case_sensitive(&right.0) || !left.1.eq_case_sensitive(&right.1) {
                        return false;
                    }
                }
                true
            }
            (Variant::Flyweight(f1), Variant::Flyweight(f2)) => {
                if f1.delegate() != f2.delegate() {
                    return false;
                }
                let slots1 = f1.slots_storage();
                let slots2 = f2.slots_storage();
                if slots1.len() != slots2.len() {
                    return false;
                }
                for ((k1, v1), (k2, v2)) in slots1.iter().zip(slots2.iter()) {
                    if k1 != k2 || !v1.eq_case_sensitive(v2) {
                        return false;
                    }
                }
                let contents1 = Var::from_list(f1.contents().clone());
                let contents2 = Var::from_list(f2.contents().clone());
                contents1.eq_case_sensitive(&contents2)
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

    // === Internal clone helper ===

    #[cold]
    #[inline(never)]
    fn clone_complex(&self) -> Self {
        match self.tag {
            // Str, List, Map, Lambda: data contains transmuted value, clone = Arc bump
            TAG_STR => {
                let s = unsafe { &*(&self.data as *const u64 as *const string::Str) };
                Self::from_str_type(s.clone())
            }
            TAG_LIST => {
                let l = unsafe { &*(&self.data as *const u64 as *const List) };
                Self::from_list(l.clone())
            }
            TAG_MAP => {
                let m = unsafe { &*(&self.data as *const u64 as *const map::Map) };
                Self::from_map(m.clone())
            }
            TAG_LAMBDA => {
                let l = unsafe { &*(&self.data as *const u64 as *const Lambda) };
                Self::from_lambda(l.clone())
            }
            // Err: data is Arc pointer
            TAG_ERR => {
                let arc = unsafe { Arc::from_raw(self.data as *const Error) };
                let cloned = Arc::clone(&arc);
                std::mem::forget(arc);
                Self {
                    tag: TAG_ERR,
                    meta: [0; 7],
                    data: Arc::into_raw(cloned) as u64,
                }
            }
            // Flyweight, Binary: data is Box pointer
            TAG_FLYWEIGHT => {
                let f = unsafe { &*(self.data as *const Flyweight) };
                Self::from_flyweight(f.clone())
            }
            TAG_BINARY => {
                let b = unsafe { &*(self.data as *const Binary) };
                let boxed = Box::new(b.clone());
                Self {
                    tag: TAG_BINARY,
                    meta: self.meta, // preserve cached length
                    data: Box::into_raw(boxed) as u64,
                }
            }
            _ => unreachable!("clone_complex called on simple type"),
        }
    }
}

// === Clone, Drop, and standard traits ===

impl Clone for Var {
    #[inline]
    fn clone(&self) -> Self {
        if self.is_simple() {
            // SAFETY: For simple types (no heap allocation), we can just copy the bytes
            unsafe { std::ptr::read(self) }
        } else {
            self.clone_complex()
        }
    }
}

impl Drop for Var {
    fn drop(&mut self) {
        if self.is_simple() {
            return;
        }
        match self.tag {
            // Str, List, Map, Lambda: data contains transmuted value, drop by transmuting back
            TAG_STR => {
                let _ = unsafe { std::mem::transmute::<u64, string::Str>(self.data) };
            }
            TAG_LIST => {
                let _ = unsafe { std::mem::transmute::<u64, List>(self.data) };
            }
            TAG_MAP => {
                let _ = unsafe { std::mem::transmute::<u64, map::Map>(self.data) };
            }
            TAG_LAMBDA => {
                let _ = unsafe { std::mem::transmute::<u64, Lambda>(self.data) };
            }
            // Err: data is Arc pointer
            TAG_ERR => {
                let _ = unsafe { Arc::from_raw(self.data as *const Error) };
            }
            // Flyweight, Binary: data is Box pointer
            TAG_FLYWEIGHT => {
                let _ = unsafe { Box::from_raw(self.data as *mut Flyweight) };
            }
            TAG_BINARY => {
                let _ = unsafe { Box::from_raw(self.data as *mut Binary) };
            }
            _ => {}
        }
    }
}

impl Debug for Var {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.variant() {
            Variant::None => write!(f, "None"),
            Variant::Bool(b) => write!(f, "{b}"),
            Variant::Obj(o) => write!(f, "Object({o})"),
            Variant::Int(i) => write!(f, "Integer({i})"),
            Variant::Float(fl) => write!(f, "Float({fl})"),
            Variant::List(l) => {
                let items: Vec<_> = l.iter().collect();
                write!(f, "List([size = {}, items = {items:?}])", l.len())
            }
            Variant::Str(s) => write!(f, "String({:?})", s.as_str()),
            Variant::Map(m) => {
                let items: Vec<_> = m.iter().collect();
                write!(f, "Map([size = {}, items = {items:?}])", m.len())
            }
            Variant::Err(e) => write!(f, "Error({e:?})"),
            Variant::Flyweight(fl) => write!(f, "Flyweight({fl:?})"),
            Variant::Sym(s) => write!(f, "Symbol({s})"),
            Variant::Binary(b) => write!(f, "Binary({} bytes)", b.len()),
            Variant::Lambda(l) => {
                use crate::program::opcode::ScatterLabel;
                let param_str =
                    l.0.params
                        .labels
                        .iter()
                        .map(|label| match label {
                            ScatterLabel::Required(_) => "x",
                            ScatterLabel::Optional(_, _) => "?x",
                            ScatterLabel::Rest(_) => "@x",
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                write!(f, "Lambda(({param_str}))")
            }
        }
    }
}

impl Hash for Var {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.variant() {
            Variant::None => 0.hash(state),
            Variant::Bool(b) => b.hash(state),
            Variant::Obj(o) => o.hash(state),
            Variant::Int(i) => i.hash(state),
            Variant::Float(f) => f.to_bits().hash(state),
            Variant::List(l) => l.hash(state),
            Variant::Str(s) => s.hash(state),
            Variant::Map(m) => m.hash(state),
            Variant::Err(e) => e.hash(state),
            Variant::Flyweight(f) => f.hash(state),
            Variant::Sym(s) => s.hash(state),
            Variant::Binary(b) => b.hash(state),
            Variant::Lambda(l) => std::ptr::hash(&*l.0.body.0, state),
        }
    }
}

impl PartialEq for Var {
    fn eq(&self, other: &Self) -> bool {
        if self.tag != other.tag {
            let self_is_str = matches!(self.tag, TAG_STR | TAG_EMPTY_STR | TAG_SYMBOL_STR);
            let other_is_str = matches!(other.tag, TAG_STR | TAG_EMPTY_STR | TAG_SYMBOL_STR);
            if self_is_str && other_is_str {
                return self.as_str().unwrap() == other.as_str().unwrap();
            }
            let self_is_list = matches!(self.tag, TAG_LIST | TAG_EMPTY_LIST);
            let other_is_list = matches!(other.tag, TAG_LIST | TAG_EMPTY_LIST);
            if self_is_list && other_is_list {
                return self.as_list().unwrap() == other.as_list().unwrap();
            }
            return false;
        }
        // Tags match, compare data directly based on tag
        match self.tag {
            TAG_NONE => true,
            TAG_BOOL_FALSE | TAG_BOOL_TRUE => true, // tags already match
            TAG_INT | TAG_OBJ | TAG_SYM => self.data == other.data,
            TAG_FLOAT => {
                // Float comparison needs special handling for NaN
                let l = f64::from_bits(self.data);
                let r = f64::from_bits(other.data);
                l == r
            }
            // Complex types - delegate to their PartialEq
            TAG_EMPTY_STR => true,
            TAG_SYMBOL_STR => self.as_str().unwrap() == other.as_str().unwrap(),
            TAG_STR => self.as_str().unwrap() == other.as_str().unwrap(),
            TAG_EMPTY_LIST => true,
            TAG_LIST => self.as_list().unwrap() == other.as_list().unwrap(),
            TAG_MAP => self.as_map().unwrap() == other.as_map().unwrap(),
            TAG_ERR => self.as_error().unwrap() == other.as_error().unwrap(),
            TAG_FLYWEIGHT => self.as_flyweight().unwrap() == other.as_flyweight().unwrap(),
            TAG_BINARY => self.as_binary().unwrap() == other.as_binary().unwrap(),
            TAG_LAMBDA => self.as_lambda().unwrap() == other.as_lambda().unwrap(),
            _ => unreachable!("invalid tag"),
        }
    }
}

impl Eq for Var {}

impl Ord for Var {
    fn cmp(&self, other: &Self) -> Ordering {
        // Fast path: int cmp int (most common)
        if self.tag == TAG_INT && other.tag == TAG_INT {
            return (self.data as i64).cmp(&(other.data as i64));
        }

        // Fast path: float cmp float
        if self.tag == TAG_FLOAT && other.tag == TAG_FLOAT {
            return f64::from_bits(self.data).total_cmp(&f64::from_bits(other.data));
        }

        // Fast path: int cmp float / float cmp int
        if self.tag == TAG_INT && other.tag == TAG_FLOAT {
            return (self.data as i64 as f64).total_cmp(&f64::from_bits(other.data));
        }
        if self.tag == TAG_FLOAT && other.tag == TAG_INT {
            return f64::from_bits(self.data).total_cmp(&(other.data as i64 as f64));
        }

        // Slow path for other types
        self.cmp_slow(other)
    }
}

impl Var {
    #[inline(never)]
    fn cmp_slow(&self, other: &Self) -> Ordering {
        match (self.variant(), other.variant()) {
            (Variant::None, Variant::None) => Ordering::Equal,
            (Variant::Bool(l), Variant::Bool(r)) => l.cmp(&r),
            (Variant::Obj(l), Variant::Obj(r)) => l.cmp(&r),
            (Variant::Int(l), Variant::Int(r)) => l.cmp(&r),
            (Variant::Float(l), Variant::Float(r)) => l.total_cmp(&r),
            (Variant::List(l), Variant::List(r)) => l.cmp(r),
            (Variant::Str(l), Variant::Str(r)) => l.cmp(r),
            (Variant::Map(l), Variant::Map(r)) => l.cmp(r),
            (Variant::Err(l), Variant::Err(r)) => l.cmp(r),
            (Variant::Flyweight(l), Variant::Flyweight(r)) => l.cmp(r),
            (Variant::Sym(l), Variant::Sym(r)) => l.cmp(&r),
            (Variant::Binary(l), Variant::Binary(r)) => l.cmp(r),
            (Variant::Lambda(l), Variant::Lambda(r)) => {
                use crate::program::program::PrgInner;
                let l_ptr = &*l.0.body.0 as *const PrgInner;
                let r_ptr = &*r.0.body.0 as *const PrgInner;
                l_ptr.cmp(&r_ptr)
            }
            (Variant::Int(l), Variant::Float(r)) => (l as f64).total_cmp(&r),
            (Variant::Float(l), Variant::Int(r)) => l.total_cmp(&(r as f64)),
            (Variant::None, _) => Ordering::Less,
            (_, Variant::None) => Ordering::Greater,
            (Variant::Bool(_), _) => Ordering::Less,
            (_, Variant::Bool(_)) => Ordering::Greater,
            (Variant::Obj(_), _) => Ordering::Less,
            (_, Variant::Obj(_)) => Ordering::Greater,
            (Variant::Int(_), _) => Ordering::Less,
            (_, Variant::Int(_)) => Ordering::Greater,
            (Variant::Float(_), _) => Ordering::Less,
            (_, Variant::Float(_)) => Ordering::Greater,
            (Variant::List(_), _) => Ordering::Less,
            (_, Variant::List(_)) => Ordering::Greater,
            (Variant::Str(_), _) => Ordering::Less,
            (_, Variant::Str(_)) => Ordering::Greater,
            (Variant::Map(_), _) => Ordering::Less,
            (_, Variant::Map(_)) => Ordering::Greater,
            (Variant::Flyweight(_), _) => Ordering::Less,
            (_, Variant::Flyweight(_)) => Ordering::Greater,
            (Variant::Sym(_), _) => Ordering::Less,
            (_, Variant::Sym(_)) => Ordering::Greater,
            (Variant::Binary(_), _) => Ordering::Less,
            (_, Variant::Binary(_)) => Ordering::Greater,
            (Variant::Lambda(_), _) => Ordering::Greater,
            (_, Variant::Lambda(_)) => Ordering::Less,
        }
    }
}

impl PartialOrd for Var {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl ByteSized for Var {
    fn size_bytes(&self) -> usize {
        match self.variant() {
            Variant::List(l) => l.iter().map(|e| e.size_bytes()).sum::<usize>(),
            Variant::Str(s) => s.as_str().len(),
            Variant::Map(m) => m
                .iter()
                .map(|(k, v)| k.size_bytes() + v.size_bytes())
                .sum::<usize>(),
            Variant::Err(e) => {
                e.msg.as_ref().map(|s| s.len()).unwrap_or(0)
                    + e.value.as_ref().map(|s| s.size_bytes()).unwrap_or(0)
                    + size_of::<crate::ErrorCode>()
            }
            Variant::Flyweight(f) => {
                size_of::<Obj>()
                    + f.contents().iter().map(|e| e.size_bytes()).sum::<usize>()
                    + f.slots_storage()
                        .iter()
                        .map(|(_, v)| size_of::<Symbol>() + v.size_bytes())
                        .sum::<usize>()
            }
            Variant::Binary(b) => b.as_byte_view().len(),
            Variant::Lambda(l) => size_of_val(l),
            _ => size_of::<Var>(),
        }
    }
}

// === From implementations ===

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

// === Constructor functions ===

pub fn v_int(i: i64) -> Var {
    Var::mk_integer(i)
}

/// Produces a truthy integer, not a boolean, for LambdaMOO compatibility.
pub fn v_bool_int(b: bool) -> Var {
    if b { v_int(1) } else { v_int(0) }
}

pub fn v_bool(b: bool) -> Var {
    Var::mk_bool(b)
}

pub fn v_none() -> Var {
    Var::mk_none()
}

pub fn v_str(s: &str) -> Var {
    Var::mk_str(s)
}

pub fn v_string(s: String) -> Var {
    Var::mk_str(&s)
}

pub fn v_arc_str(s: arcstr::ArcStr) -> Var {
    let str_val = crate::string::Str::mk_arc_str(s);
    Var::from_str_type(str_val)
}

pub fn v_symbol_str(symbol: Symbol) -> Var {
    Var::from_symbol_str(symbol)
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
    Var::from_flyweight(fl)
}

pub fn v_empty_list() -> Var {
    EMPTY_LIST_VAR.clone()
}

pub fn v_empty_str() -> Var {
    EMPTY_STR_VAR.clone()
}

/// Return cached NOTHING object Var.
pub fn v_nothing() -> Var {
    NOTHING_VAR.clone()
}

pub fn v_empty_map() -> Var {
    v_map(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size() {
        assert_eq!(std::mem::size_of::<Var>(), 16);
    }

    #[test]
    fn test_simple_clone() {
        let v = Var::mk_integer(42);
        let c = v.clone();
        match c.variant() {
            Variant::Int(i) => assert_eq!(i, 42),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn test_bool() {
        let t = Var::mk_bool(true);
        let f = Var::mk_bool(false);
        match t.variant() {
            Variant::Bool(b) => assert!(b),
            _ => panic!("wrong type"),
        }
        match f.variant() {
            Variant::Bool(b) => assert!(!b),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn test_equality() {
        assert_eq!(Var::mk_integer(42), Var::mk_integer(42));
        assert_ne!(Var::mk_integer(42), Var::mk_integer(43));
        assert_eq!(Var::mk_bool(true), Var::mk_bool(true));
        assert_ne!(Var::mk_bool(true), Var::mk_bool(false));
    }

    #[test]
    fn test_ordering() {
        assert!(Var::mk_integer(1) < Var::mk_integer(2));
        assert!(Var::mk_none() < Var::mk_integer(0));
    }

    #[test]
    fn test_int_pack_unpack() {
        let i = Var::mk_integer(42);
        match i.variant() {
            Variant::Int(i) => assert_eq!(i, 42),
            _ => panic!("Expected integer"),
        }
    }

    #[test]
    fn test_float_pack_unpack() {
        let f = Var::mk_float(42.0);
        match f.variant() {
            Variant::Float(f) => assert_eq!(f, 42.0),
            _ => panic!("Expected float"),
        }
    }

    #[test]
    fn test_alpha_numeric_sort_order() {
        let six = Var::mk_integer(6);
        let a = Var::mk_str("a");
        assert_eq!(six.cmp(&a), std::cmp::Ordering::Less);

        let nine = Var::mk_integer(9);
        assert_eq!(nine.cmp(&a), std::cmp::Ordering::Less);

        assert_eq!(a.cmp(&six), std::cmp::Ordering::Greater);
        assert_eq!(a.cmp(&nine), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_var_size() {
        assert!(
            size_of::<Var>() <= 16,
            "Var size exceeds 128 bits: {}",
            size_of::<Var>()
        );
    }
}
