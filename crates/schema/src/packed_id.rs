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

//! Packed time-based ID helpers for UuObjid and AnonymousObjid.
//!
//! Both ID types use the same 62-bit packed format:
//! `[autoincrement (16)] [rng (6)] [epoch_ms (40)]`
//!
//! This module provides common pack/unpack functions to avoid duplication.

/// Pack components into a 62-bit value.
///
/// Layout: `[autoincrement (16)] [rng (6)] [epoch_ms (40)]`
#[inline]
pub fn pack_time_id(autoincrement: u16, rng: u8, epoch_ms: u64) -> u64 {
    ((autoincrement as u64) << 46) | ((rng as u64) << 40) | (epoch_ms & 0x00FF_FFFF_FFFF)
}

/// Unpack a 62-bit value into components.
///
/// Returns `(autoincrement, rng, epoch_ms)`.
#[inline]
pub fn unpack_time_id(packed: u64) -> (u16, u8, u64) {
    let autoincrement = ((packed >> 46) & 0xFFFF) as u16;
    let rng = ((packed >> 40) & 0x3F) as u8;
    let epoch_ms = packed & 0x00FF_FFFF_FFFF;
    (autoincrement, rng, epoch_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_roundtrip() {
        let autoincrement = 0x1234u16;
        let rng = 0x15u8; // 6 bits max
        let epoch_ms = 0x00AB_CDEF_1234u64;

        let packed = pack_time_id(autoincrement, rng, epoch_ms);
        let (a, r, e) = unpack_time_id(packed);

        assert_eq!(a, autoincrement);
        assert_eq!(r, rng);
        assert_eq!(e, epoch_ms);
    }

    #[test]
    fn test_max_values() {
        let autoincrement = 0xFFFFu16;
        let rng = 0x3Fu8; // Max 6-bit value
        let epoch_ms = 0x00FF_FFFF_FFFFu64; // Max 40-bit value

        let packed = pack_time_id(autoincrement, rng, epoch_ms);
        let (a, r, e) = unpack_time_id(packed);

        assert_eq!(a, autoincrement);
        assert_eq!(r, rng);
        assert_eq!(e, epoch_ms);
    }

    #[test]
    fn test_zero_values() {
        let packed = pack_time_id(0, 0, 0);
        let (a, r, e) = unpack_time_id(packed);

        assert_eq!(a, 0);
        assert_eq!(r, 0);
        assert_eq!(e, 0);
    }
}
