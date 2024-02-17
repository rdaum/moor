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

use moor_values::util::BitsetTrait;

#[cfg(feature = "simd_keys")]
mod simd_keys {
    use simdeez::*;
    use simdeez::{prelude::*, simd_runtime_generate};

    simd_runtime_generate!(
        pub fn simdeez_find_insert_pos(key: u8, keys: &[u8], ff_mask_out: u32) -> Option<usize> {
            let key_cmp_vec = S::Vi8::set1(key as i8);
            let key_vec = SimdBaseIo::load_from_ptr_unaligned(keys.as_ptr() as *const i8);
            let results = key_cmp_vec.cmp_lt(key_vec);
            let bitfield = results.get_mask() & (ff_mask_out as u32);
            if bitfield != 0 {
                let idx = bitfield.trailing_zeros() as usize;
                return Some(idx);
            }
            None
        }
    );

    simd_runtime_generate!(
        pub fn simdeez_find_key(key: u8, keys: &[u8], ff_mask_out: u32) -> Option<usize> {
            let key_cmp_vec = S::Vi8::set1(key as i8);
            let key_vec = SimdBaseIo::load_from_ptr_unaligned(keys.as_ptr() as *const i8);
            let results = key_cmp_vec.cmp_eq(key_vec);
            let bitfield = results.get_mask() & (ff_mask_out as u32);
            if bitfield != 0 {
                let idx = bitfield.trailing_zeros() as usize;
                return Some(idx);
            }
            None
        }
    );
}

#[cfg(not(feature = "simd_keys"))]
fn binary_find_key(key: u8, keys: &[u8], num_children: usize) -> Option<usize> {
    let mut left = 0;
    let mut right = num_children;
    while left < right {
        let mid = (left + right) / 2;
        match keys[mid].cmp(&key) {
            std::cmp::Ordering::Less => left = mid + 1,
            std::cmp::Ordering::Equal => return Some(mid),
            std::cmp::Ordering::Greater => right = mid,
        }
    }
    None
}

pub fn u8_keys_find_key_position_sorted<const WIDTH: usize>(
    key: u8,
    keys: &[u8],
    num_children: usize,
) -> Option<usize> {
    // Width 4 and under, just use linear search.
    if WIDTH <= 4 {
        return (0..num_children).find(|&i| keys[i] == key);
    }

    #[cfg(feature = "simd_keys")]
    {
        simd_keys::simdeez_find_key(key, keys, (1 << num_children) - 1)
    }

    // Fallback to binary search.
    #[cfg(not(feature = "simd_keys"))]
    binary_find_key(key, keys, num_children)
}

pub fn u8_keys_find_insert_position_sorted<const WIDTH: usize>(
    key: u8,
    keys: &[u8],
    num_children: usize,
) -> Option<usize> {
    #[cfg(feature = "simd_keys")]
    {
        simd_keys::simdeez_find_insert_pos(key, keys, (1 << num_children) - 1)
            .or(Some(num_children))
    }

    // Fallback: use linear search to find the insertion point.
    #[cfg(not(feature = "simd_keys"))]
    (0..num_children)
        .find(|&i| key < keys[i])
        .or(Some(num_children))
}

#[allow(dead_code)]
pub fn u8_keys_find_key_position<const WIDTH: usize, Bitset: BitsetTrait>(
    key: u8,
    keys: &[u8],
    children_bitmask: &Bitset,
) -> Option<usize> {
    // SIMD optimized
    #[cfg(feature = "simd_keys")]
    {
        // Special 0xff key is special
        let mut mask = (1 << WIDTH) - 1;
        if key == 255 {
            mask &= children_bitmask.as_bitmask() as u32;
        }
        simd_keys::simdeez_find_key(key, keys, mask)
    }

    #[cfg(not(feature = "simd_keys"))]
    {
        // Fallback to linear search for non-SIMD.
        for (i, k) in keys.iter().enumerate() {
            if key == 255 && !children_bitmask.check(i) {
                continue;
            }
            if *k == key {
                return Some(i);
            }
        }
        None
    }
}
