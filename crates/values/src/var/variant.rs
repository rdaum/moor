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

use bincode::{Decode, Encode};

use crate::var::error::Error;
use crate::var::list::List;
use crate::var::objid::Objid;
use crate::var::string::Str;
use std::fmt::{Display, Formatter, Result as FmtResult};

use super::Var;

#[derive(Clone, Encode, Decode, Debug)]
pub enum Variant {
    None,
    Str(Str),
    Obj(Objid),
    Int(i64),
    Float(f64),
    Err(Error),
    List(List),
}

impl Display for Variant {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::None => write!(f, "None"),
            Self::Str(s) => write!(f, "{s}"),
            Self::Obj(o) => write!(f, "{o}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(fl) => write!(f, "{fl}"),
            Self::Err(e) => write!(f, "{e}"),
            Self::List(l) => write!(f, "{l}"),
        }
    }
}

impl From<Variant> for Var {
    fn from(val: Variant) -> Self {
        Self::new(val)
    }
}
