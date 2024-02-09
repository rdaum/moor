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

use crate::encode::{DecodingError, EncodingError};
use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Encode, Decode)]
pub struct Objid(pub i64);

impl LayoutAs<i64> for Objid {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: i64) -> Result<Self, Self::ReadError> {
        Ok(Self(v))
    }

    fn try_write(v: Self) -> Result<i64, Self::WriteError> {
        Ok(v.0)
    }
}

impl Display for Objid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#{}", self.0))
    }
}

impl Objid {
    #[must_use]
    pub fn to_literal(&self) -> String {
        format!("#{}", self.0)
    }

    #[must_use]
    pub fn is_sysobj(&self) -> bool {
        self.0 == 0
    }
}
