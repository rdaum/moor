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

use std::fmt::{Debug, Display, Formatter};

use binary_layout::LayoutAs;
use bincode::{Decode, Encode};
use bytes::Bytes;

use crate::encode::{DecodingError, EncodingError};
use crate::AsByteBuffer;

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

impl AsByteBuffer for Objid {
    fn size_bytes(&self) -> usize {
        8
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(&self.0.to_le_bytes()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        let bytes = bytes.as_ref();
        if bytes.len() != 8 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 8 bytes, got {}",
                bytes.len()
            )));
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(bytes);
        Ok(Self(i64::from_le_bytes(buf)))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(Bytes::from(self.make_copy_as_vec()?))
    }
}

impl TryFrom<&str> for Objid {
    type Error = DecodingError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(value) =value.strip_prefix('#') {
            let value = value.parse::<i64>().map_err(|e| {
                DecodingError::CouldNotDecode(format!("Could not parse Objid: {}", e))
            })?;
            Ok(Self(value))
        } else {
            Err(DecodingError::CouldNotDecode(format!(
                "Expected Objid to start with '#', got {}",
                value
            )))
        }
    }
}
