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

use std::{
    marker::PhantomData,
    ops::{BitOr, BitOrAssign},
};

use moor_var::encode::{DecodingError, EncodingError};

use moor_var::ByteSized;
/// Compact flag storage for small enum-based bitsets.
///
/// `BitEnum` stores up to 16 flag bits in a `u16`. It is intended for dense flag enums whose
/// discriminants are in the range `0..16`.
///
/// Callers should use flag-type-specific constructors for "all valid flags" semantics. The
/// representation-level `all_bits()` constructor sets every storage bit, including bits that may
/// not correspond to real enum variants.
///
/// This remains a custom type because it participates in the project's binary encodings and byte
/// views.
use zerocopy::{FromBytes, Immutable, IntoBytes};

pub trait BitFlag {
    fn bit_index(self) -> u8;
}

#[derive(
    Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash, FromBytes, Immutable, IntoBytes,
)]
#[repr(transparent)]
pub struct BitEnum<T: BitFlag> {
    value: u16,
    phantom: PhantomData<T>,
}

impl<T: BitFlag> BitEnum<T> {
    pub fn try_read(v: u16) -> Result<Self, DecodingError> {
        Ok(Self {
            value: v,
            phantom: PhantomData,
        })
    }

    pub fn try_write(v: Self) -> Result<u16, EncodingError> {
        Ok(v.to_u16())
    }

    #[inline]
    fn bit(value: T) -> u16 {
        let bit = u32::from(value.bit_index());
        debug_assert!(bit < u16::BITS, "BitEnum discriminant out of range: {bit}");
        1u16 << bit
    }

    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: 0,
            phantom: PhantomData,
        }
    }
    #[inline]
    #[must_use]
    pub fn to_u16(&self) -> u16 {
        self.value
    }

    #[inline]
    #[must_use]
    pub fn from_u8(value: u8) -> Self {
        Self {
            value: u16::from(value),
            phantom: PhantomData,
        }
    }

    #[inline]
    #[must_use]
    pub fn from_u16(value: u16) -> Self {
        Self {
            value,
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn new_with(value: T) -> Self {
        let mut s = Self {
            value: 0,
            phantom: PhantomData,
        };
        s.set(value);
        s
    }

    #[inline]
    #[must_use]
    pub fn all_bits() -> Self {
        Self {
            value: u16::MAX,
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn set(&mut self, value: T) {
        self.value |= Self::bit(value);
    }

    #[inline]
    pub fn clear(&mut self, value: T) {
        self.value &= !Self::bit(value);
    }

    #[inline]
    pub fn contains(&self, value: T) -> bool {
        self.value & Self::bit(value) != 0
    }

    #[inline]
    pub fn contains_all(&self, values: BitEnum<T>) -> bool {
        // Verify that all bits from common are in self.value
        values.value & self.value == values.value
    }
}

impl<T: BitFlag> BitOr for BitEnum<T> {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value | rhs.value,
            phantom: PhantomData,
        }
    }
}

impl<T: BitFlag> Default for BitEnum<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: BitFlag> BitOrAssign<T> for BitEnum<T> {
    fn bitor_assign(&mut self, rhs: T) {
        self.set(rhs);
    }
}

impl<T: BitFlag> BitOr<T> for BitEnum<T> {
    type Output = Self;

    fn bitor(self, rhs: T) -> Self::Output {
        let mut s = self;
        s.set(rhs);
        s
    }
}

impl<T: BitFlag> From<T> for BitEnum<T> {
    fn from(value: T) -> Self {
        Self::new_with(value)
    }
}

impl<T: BitFlag> ByteSized for BitEnum<T> {
    fn size_bytes(&self) -> usize {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::{BitEnum, BitFlag};

    #[derive(Clone, Copy)]
    enum DemoFlag {
        A = 0,
        B = 1,
        C = 2,
    }

    impl BitFlag for DemoFlag {
        fn bit_index(self) -> u8 {
            self as u8
        }
    }

    #[test]
    fn stores_and_queries_flags() {
        let flags = BitEnum::new_with(DemoFlag::A) | DemoFlag::C;
        assert!(flags.contains(DemoFlag::A));
        assert!(!flags.contains(DemoFlag::B));
        assert!(flags.contains(DemoFlag::C));
    }

    #[test]
    fn contains_all_requires_all_requested_bits() {
        let flags = BitEnum::new_with(DemoFlag::A) | DemoFlag::B;
        assert!(flags.contains_all(BitEnum::new_with(DemoFlag::A)));
        assert!(flags.contains_all(BitEnum::new_with(DemoFlag::A) | DemoFlag::B));
        assert!(!flags.contains_all(BitEnum::new_with(DemoFlag::C)));
    }

    #[test]
    fn raw_constructors_round_trip() {
        assert_eq!(BitEnum::<DemoFlag>::from_u8(0b101).to_u16(), 0b101);
        assert_eq!(BitEnum::<DemoFlag>::from_u16(0x00ff).to_u16(), 0x00ff);
    }

    #[test]
    fn all_bits_sets_storage_mask() {
        assert_eq!(BitEnum::<DemoFlag>::all_bits().to_u16(), u16::MAX);
    }
}
