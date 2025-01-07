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

use std::cmp::min;
use std::ops::Index;

use num_traits::PrimInt;

pub trait BitsetTrait: Default {
    // Total size of the bitset in bits.
    const BITSET_WIDTH: usize;
    // Total size of the bitset in bytes.
    const STORAGE_WIDTH_BYTES: usize;
    // Bit shift factor -- e.g. 3 for 8, 4 for 16, etc.
    const BIT_SHIFT: usize;
    // Bit width of each storage unit.
    const STORAGE_BIT_WIDTH: usize;
    // Total size of storage in its internal storage width (e.g. u16, u32, etc.)
    const STORAGE_WIDTH: usize;

    fn first_empty(&self) -> Option<usize>;
    fn first_set(&self) -> Option<usize>;
    fn set(&mut self, pos: usize);
    fn unset(&mut self, pos: usize);
    fn check(&self, pos: usize) -> bool;
    fn clear(&mut self);
    fn last(&self) -> Option<usize>;
    fn is_empty(&self) -> bool;
    fn size(&self) -> usize;
    fn bit_width(&self) -> usize;
    fn capacity(&self) -> usize;
    fn storage_width(&self) -> usize;
    fn as_bitmask(&self) -> u128;
}

pub struct Bitset<StorageType, const STORAGE_WIDTH: usize>
where
    StorageType: PrimInt,
{
    bitset: [StorageType; STORAGE_WIDTH],
}

impl<StorageType, const STORAGE_WIDTH: usize> Bitset<StorageType, STORAGE_WIDTH>
where
    StorageType: PrimInt,
{
    pub fn new() -> Self {
        Self {
            bitset: [StorageType::min_value(); STORAGE_WIDTH],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.bitset.iter().enumerate().flat_map(|(i, b)| {
            (0..Self::STORAGE_BIT_WIDTH).filter_map(move |j| {
                let b: u64 = b.to_u64().unwrap();
                if (b) & (1 << j) != 0 {
                    Some((i << Self::BIT_SHIFT) + j)
                } else {
                    None
                }
            })
        })
    }
}

impl<StorageType, const STORAGE_WIDTH: usize> BitsetTrait for Bitset<StorageType, STORAGE_WIDTH>
where
    StorageType: PrimInt,
{
    const BITSET_WIDTH: usize = Self::STORAGE_BIT_WIDTH * STORAGE_WIDTH;
    const STORAGE_WIDTH_BYTES: usize = Self::BITSET_WIDTH / 8;
    const BIT_SHIFT: usize = Self::STORAGE_BIT_WIDTH.trailing_zeros() as usize;
    const STORAGE_BIT_WIDTH: usize = std::mem::size_of::<StorageType>() * 8;
    const STORAGE_WIDTH: usize = STORAGE_WIDTH;

    fn first_empty(&self) -> Option<usize> {
        for (i, b) in self.bitset.iter().enumerate() {
            if b.is_zero() {
                return Some(i << Self::BIT_SHIFT);
            }
            if *b != StorageType::max_value() {
                return Some((i << Self::BIT_SHIFT) + b.trailing_ones() as usize);
            }
        }
        None
    }

    fn first_set(&self) -> Option<usize> {
        for (i, b) in self.bitset.iter().enumerate() {
            if !b.is_zero() {
                return Some((i << Self::BIT_SHIFT) + b.trailing_zeros() as usize);
            }
        }
        None
    }

    #[inline]
    fn set(&mut self, pos: usize) {
        assert!(pos < Self::BITSET_WIDTH);
        let v = self.bitset[pos >> Self::BIT_SHIFT];
        let shift: StorageType = StorageType::one() << (pos % Self::STORAGE_BIT_WIDTH);
        let v = v.bitor(shift);
        self.bitset[pos >> Self::BIT_SHIFT] = v;
    }

    #[inline]
    fn unset(&mut self, pos: usize) {
        assert!(pos < Self::BITSET_WIDTH);
        let v = self.bitset[pos >> Self::BIT_SHIFT];
        let shift = StorageType::one() << (pos % Self::STORAGE_BIT_WIDTH);
        let v = v & shift.not();
        self.bitset[pos >> Self::BIT_SHIFT] = v;
    }

    #[inline]
    fn check(&self, pos: usize) -> bool {
        assert!(pos < Self::BITSET_WIDTH);
        let shift: StorageType = StorageType::one() << (pos % Self::STORAGE_BIT_WIDTH);
        !(self.bitset[pos >> Self::BIT_SHIFT] & shift).is_zero()
    }

    #[inline]
    fn clear(&mut self) {
        self.bitset.fill(StorageType::zero());
    }

    #[inline]
    fn last(&self) -> Option<usize> {
        for (i, b) in self.bitset.iter().enumerate() {
            if !b.is_zero() {
                return Some(
                    (i << Self::BIT_SHIFT) + (Self::STORAGE_BIT_WIDTH - 1)
                        - b.leading_zeros() as usize,
                );
            }
        }
        None
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.bitset.iter().all(|x| x.is_zero())
    }

    #[inline]
    fn size(&self) -> usize {
        self.bitset.iter().map(|x| x.count_ones() as usize).sum()
    }

    #[inline]
    fn bit_width(&self) -> usize {
        Self::STORAGE_BIT_WIDTH
    }

    #[inline]
    fn capacity(&self) -> usize {
        Self::BITSET_WIDTH
    }

    #[inline]
    fn storage_width(&self) -> usize {
        Self::STORAGE_WIDTH
    }

    fn as_bitmask(&self) -> u128 {
        assert!(Self::STORAGE_BIT_WIDTH <= 128);
        let mut mask = 0u128;
        // copy bit-level representation, unsafe ptr copy
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.bitset.as_ptr() as *const u8,
                &mut mask as *mut u128 as *mut u8,
                min(16, Self::STORAGE_WIDTH_BYTES),
            );
        }
        mask
    }
}

impl<StorageType, const STORAGE_WIDTH: usize> Default for Bitset<StorageType, STORAGE_WIDTH>
where
    StorageType: PrimInt,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<StorageType, const STORAGE_WIDTH: usize> Index<usize> for Bitset<StorageType, STORAGE_WIDTH>
where
    StorageType: PrimInt,
{
    type Output = bool;

    #[inline]
    fn index(&self, pos: usize) -> &Self::Output {
        if self.check(pos) {
            &true
        } else {
            &false
        }
    }
}

pub type Bitset64<const STORAGE_WIDTH_U64: usize> = Bitset<u64, STORAGE_WIDTH_U64>;
pub type Bitset32<const STORAGE_WIDTH_U32: usize> = Bitset<u32, STORAGE_WIDTH_U32>;
pub type Bitset16<const STORAGE_WIDTH_U16: usize> = Bitset<u16, STORAGE_WIDTH_U16>;
pub type Bitset8<const STORAGE_WIDTH_U8: usize> = Bitset<u8, STORAGE_WIDTH_U8>;

#[cfg(test)]
mod tests {
    use crate::util::BitsetTrait;

    #[test]
    fn test_first_free_8s() {
        let mut bs = super::Bitset8::<4>::new();
        bs.set(1);
        bs.set(3);
        assert_eq!(bs.first_empty(), Some(0));
        bs.set(0);
        assert_eq!(bs.first_empty(), Some(2));

        // Now fill it up and verify none.
        for i in 0..bs.capacity() {
            bs.set(i);
        }
        assert_eq!(bs.first_empty(), None);
    }

    #[test]
    fn test_first_free_8_2() {
        let mut bs = super::Bitset8::<2>::new();
        bs.set(1);
        bs.set(3);
        assert_eq!(bs.first_empty(), Some(0));
        bs.set(0);
        assert_eq!(bs.first_empty(), Some(2));

        // Now fill it up and verify none.
        for i in 0..bs.capacity() {
            bs.set(i);
        }
        assert_eq!(bs.first_empty(), None);
    }

    #[test]
    fn test_first_free_32s() {
        let mut bs = super::Bitset32::<1>::new();
        bs.set(1);
        bs.set(3);
        assert_eq!(bs.first_empty(), Some(0));
        bs.set(0);
        assert_eq!(bs.first_empty(), Some(2));

        for i in 0..bs.capacity() {
            bs.set(i);
        }
        assert_eq!(bs.first_empty(), None);
    }

    #[test]
    fn test_iter_16s() {
        let mut bs = super::Bitset16::<4>::new();
        bs.set(0);
        bs.set(1);
        bs.set(2);
        bs.set(4);
        bs.set(8);
        bs.set(16);
        let v: Vec<usize> = bs.iter().collect();
        assert_eq!(v, vec![0, 1, 2, 4, 8, 16]);
    }

    #[test]
    fn test_first_free_64s() {
        let mut bs = super::Bitset64::<4>::new();
        bs.set(1);
        bs.set(3);
        assert_eq!(bs.first_empty(), Some(0));
        bs.set(0);
        assert_eq!(bs.first_empty(), Some(2));
    }

    #[test]
    fn test_iter_64s() {
        let mut bs = super::Bitset64::<4>::new();
        bs.set(0);
        bs.set(1);
        bs.set(2);
        bs.set(4);
        bs.set(8);
        bs.set(16);
        bs.set(32);
        bs.set(47);
        bs.set(48);
        bs.set(49);
        bs.set(127);
        let v: Vec<usize> = bs.iter().collect();
        assert_eq!(v, vec![0, 1, 2, 4, 8, 16, 32, 47, 48, 49, 127]);
    }
}
