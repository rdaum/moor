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

use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};
use ustr::Ustr;

/// An interned string used for things like verb names and property names.
/// Not currently a permissible value in Var, but is used throughout the system.
/// (There will eventually be a TYPE_SYMBOL and a syntax for it in the language, but not for 1.0.)
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct Symbol(Ustr);

impl Serialize for Symbol {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_str().serialize(serializer)
    }
}

impl<'a> Deserialize<'a> for Symbol {
    fn deserialize<D: serde::Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        // Deserialize a string.
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(Symbol(Ustr::from(&s)))
    }
}

impl Symbol {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn mk(s: &str) -> Self {
        Symbol(Ustr::from(s))
    }

    pub fn mk_case_insensitive(s: &str) -> Self {
        Symbol(Ustr::from(&s.to_lowercase()))
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Symbol(Ustr::from(s))
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Debug for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Encode for Symbol {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        // Encode as string.
        let s = self.0.to_string();
        s.encode(encoder)
    }
}

impl Decode for Symbol {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        // Decode string and then intern to get the symbol
        let s: String = Decode::decode(decoder)?;
        Ok(Symbol(Ustr::from(&s)))
    }
}

impl<'de> BorrowDecode<'de> for Symbol {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let s: String = BorrowDecode::borrow_decode(decoder)?;
        Ok(Symbol(Ustr::from(&s)))
    }
}
