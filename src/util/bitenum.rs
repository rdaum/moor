use std::marker::PhantomData;
use std::ops::{BitOr, BitOrAssign};

/// A barebones minimal custom bitset enum, to replace use of EnumSet crate which was not rkyv'able.
use num_traits::ToPrimitive;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, Archive, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq, Hash,
)]
pub struct BitEnum<T: ToPrimitive> {
    value: u64,
    phantom: PhantomData<T>,
}

impl<T: ToPrimitive> BitEnum<T> {
    pub fn new() -> Self {
        Self {
            value: 0,
            phantom: PhantomData,
        }
    }
    pub fn from_u8(value: u8) -> Self {
        Self {
            value: value as u64,
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

    pub fn set(&mut self, value: T) {
        self.value |= 1 << value.to_u64().unwrap();
    }

    pub fn clear(&mut self, value: T) {
        self.value &= !(1 << value.to_u64().unwrap());
    }

    pub fn contains(&self, value: T) -> bool {
        self.value & (1 << value.to_u64().unwrap()) != 0
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
