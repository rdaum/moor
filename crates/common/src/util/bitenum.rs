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

use binary_layout::LayoutAs;
use std::marker::PhantomData;
use std::ops::{BitOr, BitOrAssign};

use crate::encode::{DecodingError, EncodingError};

use crate::AsByteBuffer;
use bincode::{Decode, Encode};
use bytes::Bytes;
/// A barebones minimal custom bitset enum, to replace use of `EnumSet` crate which was not rkyv'able.
use num_traits::ToPrimitive;

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, Encode, Decode)]
pub struct BitEnum<T: ToPrimitive> {
    value: u16,
    phantom: PhantomData<T>,
}

impl<T: ToPrimitive> LayoutAs<u16> for BitEnum<T> {
    type ReadError = DecodingError;
    type WriteError = EncodingError;

    fn try_read(v: u16) -> Result<Self, Self::ReadError> {
        Ok(Self {
            value: v,
            phantom: PhantomData,
        })
    }

    fn try_write(v: Self) -> Result<u16, Self::WriteError> {
        Ok(v.to_u16())
    }
}

impl<T: ToPrimitive> BitEnum<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: 0,
            phantom: PhantomData,
        }
    }
    #[must_use]
    pub fn to_u16(&self) -> u16 {
        self.value
    }

    #[must_use]
    pub fn from_u8(value: u8) -> Self {
        Self {
            value: u16::from(value),
            phantom: PhantomData,
        }
    }

    pub fn new_with(value: T) -> Self {
        let mut s = Self {
            value: 0,
            phantom: PhantomData,
        };
        s.set(value);
        s
    }

    #[must_use]
    pub fn all() -> Self {
        Self {
            value: u16::MAX,
            phantom: PhantomData,
        }
    }

    pub fn set(&mut self, value: T) {
        self.value |= 1 << value.to_u64().unwrap();
    }

    pub fn clear(&mut self, value: T) {
        self.value &= !(1 << value.to_u64().unwrap());
    }

    pub fn contains(&self, value: T) -> bool {
        self.value & (1 << value.to_u64().unwrap()) != 0
    }

    pub fn contains_all(&self, values: BitEnum<T>) -> bool {
        // Verify that all bits from common are in self.value
        values.value & self.value == values.value
    }
}

impl<T: ToPrimitive> BitOr for BitEnum<T> {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value | rhs.value,
            phantom: PhantomData,
        }
    }
}

impl<T: ToPrimitive> Default for BitEnum<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ToPrimitive> BitOrAssign<T> for BitEnum<T> {
    fn bitor_assign(&mut self, rhs: T) {
        self.set(rhs);
    }
}

impl<T: ToPrimitive> BitOr<T> for BitEnum<T> {
    type Output = Self;

    fn bitor(self, rhs: T) -> Self::Output {
        let mut s = self;
        s.set(rhs);
        s
    }
}

impl<T: ToPrimitive> From<T> for BitEnum<T> {
    fn from(value: T) -> Self {
        Self::new_with(value)
    }
}

impl<T: ToPrimitive> AsByteBuffer for BitEnum<T> {
    fn size_bytes(&self) -> usize {
        2
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(&self.value.to_le_bytes()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.value.to_le_bytes().to_vec())
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        let bytes = bytes.as_ref();
        if bytes.len() != 2 {
            return Err(DecodingError::CouldNotDecode(format!(
                "Expected 2 bytes, got {}",
                bytes.len()
            )));
        }
        let mut buf = [0u8; 2];
        buf.copy_from_slice(bytes);
        Ok(Self {
            value: u16::from_le_bytes(buf),
            phantom: PhantomData,
        })
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(Bytes::from(self.value.to_le_bytes().to_vec()))
    }
}
