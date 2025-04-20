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

use bincode::{Decode, Encode};
use strum::FromRepr;

/// The set of prepositions that are valid for verbs, corresponding to the set of string constants
/// defined in LambdaMOO 1.8.1.
/// TODO: Refactor/rethink preposition enum.
///   Long run a proper table with some sort of dynamic look up and a way to add new ones and
///   internationalize and so on.
#[repr(u16)]
#[derive(Copy, Clone, Debug, FromRepr, Eq, PartialEq, Hash, Encode, Decode, Ord, PartialOrd)]
pub enum Preposition {
    WithUsing = 0,
    AtTo = 1,
    InFrontOf = 2,
    IntoIn = 3,
    OnTopOfOn = 4,
    OutOf = 5,
    Over = 6,
    Through = 7,
    Under = 8,
    Behind = 9,
    Beside = 10,
    ForAbout = 11,
    Is = 12,
    As = 13,
    OffOf = 14,
}

impl Preposition {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "with/using" | "with" | "using" => Some(Self::WithUsing),
            "at/to" | "at" | "to" => Some(Self::AtTo),
            "in front of" | "in-front-of" => Some(Self::InFrontOf),
            "in/inside/into" | "in" | "inside" | "into" => Some(Self::IntoIn),
            "on top of/on/onto/upon" | "on top of" | "on" | "onto" | "upon" => {
                Some(Self::OnTopOfOn)
            }
            "out of/from inside/from" | "out of" | "from inside" | "from" => Some(Self::OutOf),
            "over" => Some(Self::Over),
            "through" => Some(Self::Through),
            "under/underneath/beneath" | "under" | "underneath" | "beneath" => Some(Self::Under),
            "behind" => Some(Self::Behind),
            "beside" => Some(Self::Beside),
            "for/about" | "for" | "about" => Some(Self::ForAbout),
            "is" => Some(Self::Is),
            "as" => Some(Self::As),
            "off/off of" | "off" | "off of" => Some(Self::OffOf),
            _ => None,
        }
    }
    pub fn to_string(&self) -> &str {
        match self {
            Self::WithUsing => "with/using",
            Self::AtTo => "at/to",
            Self::InFrontOf => "in front of",
            Self::IntoIn => "in/inside/into",
            Self::OnTopOfOn => "on top of/on/onto/upon",
            Self::OutOf => "out of/from inside/from",
            Self::Over => "over",
            Self::Through => "through",
            Self::Under => "under/underneath/beneath",
            Self::Behind => "behind",
            Self::Beside => "beside",
            Self::ForAbout => "for/about",
            Self::Is => "is",
            Self::As => "as",
            Self::OffOf => "off/off of",
        }
    }

    /// Output only one preposition, instead of the full break down.
    /// For output in objdefs, etc where space-separation is required
    pub fn to_string_single(&self) -> &str {
        match self {
            Self::WithUsing => "with",
            Self::AtTo => "at",
            Self::InFrontOf => "in-front-of",
            Self::IntoIn => "in",
            Self::OnTopOfOn => "on",
            Self::OutOf => "from",
            Self::Over => "over",
            Self::Through => "through",
            Self::Under => "under",
            Self::Behind => "behind",
            Self::Beside => "beside",
            Self::ForAbout => "for",
            Self::Is => "is",
            Self::As => "as",
            Self::OffOf => "off",
        }
    }
}

pub fn find_preposition(prep: &str) -> Option<Preposition> {
    // If the string starts with a number (with or without # prefix), treat it as a preposition ID.
    let numeric_offset = if prep.starts_with('#') { 1 } else { 0 };
    if let Ok(id) = prep[numeric_offset..].parse::<u16>() {
        return Preposition::from_repr(id);
    }

    Preposition::parse(prep)
}
