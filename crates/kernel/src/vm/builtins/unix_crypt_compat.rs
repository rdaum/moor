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

//! Classic Unix DES `crypt(3)` compatibility implementation.
//!
//! Portions of this module are adapted from the `pwhash` crate's DES `crypt`
//! implementation, which is licensed under MIT. The original notices are
//! preserved below.

// Historic Unix crypt(3) password hashing routines.
//
// Rust version Copyright (c) 2016 Ivan Nejgebauer <inejge@gmail.com>
//
// Licensed under the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>. This file may not be copied,
// modified, or distributed except according to the terms of this
// license.
//
// Original copyright/license notices follow:
//
// @(#)UnixCrypt.java   0.9 96/11/25
//
// Copyright (c) 1996 Aki Yoshida. All rights reserved.
//
// Permission to use, copy, modify and distribute this software
// for non-commercial or commercial purposes and without fee is
// hereby granted provided that this copyright notice appears in
// all copies.
// ---
// Unix crypt(3C) utility
// @version 0.9, 11/25/96
// @author  Aki Yoshida
// ---
// modified April 2001
// by Iris Van den Broeke, Daniel Deville
// ---
// Unix Crypt.
// Implements the one way cryptography used by Unix systems for
// simple password protection.
// @version $Id: UnixCrypt2.txt,v 1.1.1.1 2005/09/13 22:20:13 christos Exp $
// @author Greg Wilkins (gregw)

use std::{char, iter, str::from_utf8};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Error {
    InsufficientLength,
    EncodingError,
}

pub type Result<T> = std::result::Result<T, Error>;

pub const SALT_LEN: usize = 2;
const DES_ROUNDS: u32 = 25;

const CRYPT_HASH64: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

const CRYPT_HASH64_ENC_MAP: &[u8] = b"\x40\x40\x40\x40\x40\x40\x40\x40\x40\x40\x40\x40\x40\x40\x00\x01\
                                      \x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x40\x40\x40\x40\x40\x40\
                                      \x40\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\
                                      \x1b\x1c\x1d\x1e\x1f\x20\x21\x22\x23\x24\x25\x40\x40\x40\x40\x40\
                                      \x40\x26\x27\x28\x29\x2a\x2b\x2c\x2d\x2e\x2f\x30\x31\x32\x33\x34\
                                      \x35\x36\x37\x38\x39\x3a\x3b\x3c\x3d\x3e\x3f\x40\x40\x40\x40\x40";

fn crypt_hash64_encode(bs: &[u8]) -> String {
    let ngroups = bs.len().div_ceil(3);
    let mut out = String::with_capacity(ngroups * 4);
    for g in 0..ngroups {
        let mut g_idx = g * 3;
        let mut enc = 0u32;
        for _ in 0..3 {
            let b = if g_idx < bs.len() { bs[g_idx] } else { 0 } as u32;
            enc <<= 8;
            enc |= b;
            g_idx += 1;
        }
        for _ in 0..4 {
            out.push(char::from_u32(CRYPT_HASH64[((enc >> 18) & 0x3F) as usize] as u32).unwrap());
            enc <<= 6;
        }
    }
    match bs.len() % 3 {
        1 => {
            out.pop();
            out.pop();
        }
        2 => {
            out.pop();
        }
        _ => {}
    }
    out
}

fn decode_val(val: &str, len: usize) -> Result<u32> {
    let mut processed = 0;
    let mut s = 0u32;
    for b in val.chars() {
        let b = b as u32 - 0x20;
        if b > 0x60 {
            return Err(Error::EncodingError);
        }
        let dec = CRYPT_HASH64_ENC_MAP[b as usize];
        if dec == 64 {
            return Err(Error::EncodingError);
        }
        s >>= 6;
        s |= (dec as u32) << 26;
        processed += 1;
        if processed == len {
            break;
        }
    }
    if processed < len {
        return Err(Error::InsufficientLength);
    }
    Ok(s >> (32 - 6 * len))
}

fn encode_val(mut val: u32, mut nhex: usize) -> String {
    let mut val_arr = [0u8; 4];
    if nhex > 4 {
        nhex = 4;
    }
    let vlen = nhex;
    let mut i = 0;
    while nhex > 0 {
        nhex -= 1;
        val_arr[i] = CRYPT_HASH64[(val & 0x3F) as usize];
        val >>= 6;
        i += 1;
    }
    from_utf8(&val_arr[..vlen]).unwrap().to_owned()
}

fn secret_to_key(key: &[u8]) -> u64 {
    key.iter()
        .chain(iter::repeat(&0u8))
        .take(8)
        .fold(0u64, |kw, b| (kw << 8) | (b << 1) as u64)
}

fn do_0_crypt(keyword: u64, salt: u32, rounds: u32) -> String {
    let mut result_block = des_cipher(0, keyword, salt, rounds);
    let mut result_array = [0u8; 8];
    for i in 0..8 {
        result_array[7 - i] = (result_block & 0xFF) as u8;
        result_block >>= 8;
    }
    crypt_hash64_encode(&result_array)
}

pub fn crypt(key: &[u8], salt: &str) -> Result<String> {
    let keyword = secret_to_key(key);
    let salt_val = decode_val(salt, SALT_LEN)?;
    Ok(format!(
        "{}{}",
        encode_val(salt_val, SALT_LEN),
        do_0_crypt(keyword, salt_val, DES_ROUNDS)
    ))
}

const PC1ROT: [[u64; 16]; 16] = [
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000010000000000,
        0x0000010000000000,
        0x0000000100000000,
        0x0000000100000000,
        0x0000010100000000,
        0x0000010100000000,
        0x0000000000100000,
        0x0000000000100000,
        0x0000010000100000,
        0x0000010000100000,
        0x0000000100100000,
        0x0000000100100000,
        0x0000010100100000,
        0x0000010100100000,
    ],
    [
        0x0000000000000000,
        0x0000000080000000,
        0x0000040000000000,
        0x0000040080000000,
        0x0010000000000000,
        0x0010000080000000,
        0x0010040000000000,
        0x0010040080000000,
        0x0000000800000000,
        0x0000000880000000,
        0x0000040800000000,
        0x0000040880000000,
        0x0010000800000000,
        0x0010000880000000,
        0x0010040800000000,
        0x0010040880000000,
    ],
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000004000,
        0x0000000000004000,
        0x0000000000000008,
        0x0000000000000008,
        0x0000000000004008,
        0x0000000000004008,
        0x0000000000000010,
        0x0000000000000010,
        0x0000000000004010,
        0x0000000000004010,
        0x0000000000000018,
        0x0000000000000018,
        0x0000000000004018,
        0x0000000000004018,
    ],
    [
        0x0000000000000000,
        0x0000000200000000,
        0x0001000000000000,
        0x0001000200000000,
        0x0400000000000000,
        0x0400000200000000,
        0x0401000000000000,
        0x0401000200000000,
        0x0020000000000000,
        0x0020000200000000,
        0x0021000000000000,
        0x0021000200000000,
        0x0420000000000000,
        0x0420000200000000,
        0x0421000000000000,
        0x0421000200000000,
    ],
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000400000,
        0x0000000000400000,
        0x0000000004000000,
        0x0000000004000000,
        0x0000000004400000,
        0x0000000004400000,
        0x0000000000000800,
        0x0000000000000800,
        0x0000000000400800,
        0x0000000000400800,
        0x0000000004000800,
        0x0000000004000800,
        0x0000000004400800,
        0x0000000004400800,
    ],
    [
        0x0000000000000000,
        0x0000000000008000,
        0x0040000000000000,
        0x0040000000008000,
        0x0000004000000000,
        0x0000004000008000,
        0x0040004000000000,
        0x0040004000008000,
        0x8000000000000000,
        0x8000000000008000,
        0x8040000000000000,
        0x8040000000008000,
        0x8000004000000000,
        0x8000004000008000,
        0x8040004000000000,
        0x8040004000008000,
    ],
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000000080,
        0x0000000000000080,
        0x0000000000080000,
        0x0000000000080000,
        0x0000000000080080,
        0x0000000000080080,
        0x0000000000800000,
        0x0000000000800000,
        0x0000000000800080,
        0x0000000000800080,
        0x0000000000880000,
        0x0000000000880000,
        0x0000000000880080,
        0x0000000000880080,
    ],
    [
        0x0000000000000000,
        0x0000000008000000,
        0x0000002000000000,
        0x0000002008000000,
        0x0000100000000000,
        0x0000100008000000,
        0x0000102000000000,
        0x0000102008000000,
        0x0000200000000000,
        0x0000200008000000,
        0x0000202000000000,
        0x0000202008000000,
        0x0000300000000000,
        0x0000300008000000,
        0x0000302000000000,
        0x0000302008000000,
    ],
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000010000000,
        0x0000000010000000,
        0x0000000000001000,
        0x0000000000001000,
        0x0000000010001000,
        0x0000000010001000,
        0x0000000040000000,
        0x0000000040000000,
        0x0000000050000000,
        0x0000000050000000,
        0x0000000040001000,
        0x0000000040001000,
        0x0000000050001000,
        0x0000000050001000,
    ],
    [
        0x0000000000000000,
        0x0000001000000000,
        0x0000080000000000,
        0x0000081000000000,
        0x1000000000000000,
        0x1000001000000000,
        0x1000080000000000,
        0x1000081000000000,
        0x0004000000000000,
        0x0004001000000000,
        0x0004080000000000,
        0x0004081000000000,
        0x1004000000000000,
        0x1004001000000000,
        0x1004080000000000,
        0x1004081000000000,
    ],
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000040000,
        0x0000000000040000,
        0x0000020000000000,
        0x0000020000000000,
        0x0000020000040000,
        0x0000020000040000,
        0x0000000000000004,
        0x0000000000000004,
        0x0000000000040004,
        0x0000000000040004,
        0x0000020000000004,
        0x0000020000000004,
        0x0000020000040004,
        0x0000020000040004,
    ],
    [
        0x0000000000000000,
        0x0000400000000000,
        0x0200000000000000,
        0x0200400000000000,
        0x0080000000000000,
        0x0080400000000000,
        0x0280000000000000,
        0x0280400000000000,
        0x0000008000000000,
        0x0000408000000000,
        0x0200008000000000,
        0x0200408000000000,
        0x0080008000000000,
        0x0080408000000000,
        0x0280008000000000,
        0x0280408000000000,
    ],
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000000040,
        0x0000000000000040,
        0x0000000020000000,
        0x0000000020000000,
        0x0000000020000040,
        0x0000000020000040,
        0x0000000000200000,
        0x0000000000200000,
        0x0000000000200040,
        0x0000000000200040,
        0x0000000020200000,
        0x0000000020200000,
        0x0000000020200040,
        0x0000000020200040,
    ],
    [
        0x0000000000000000,
        0x0002000000000000,
        0x0800000000000000,
        0x0802000000000000,
        0x0100000000000000,
        0x0102000000000000,
        0x0900000000000000,
        0x0902000000000000,
        0x4000000000000000,
        0x4002000000000000,
        0x4800000000000000,
        0x4802000000000000,
        0x4100000000000000,
        0x4102000000000000,
        0x4900000000000000,
        0x4902000000000000,
    ],
    [
        0x0000000000000000,
        0x0000000000000000,
        0x0000000000002000,
        0x0000000000002000,
        0x0000000000000020,
        0x0000000000000020,
        0x0000000000002020,
        0x0000000000002020,
        0x0000000000000400,
        0x0000000000000400,
        0x0000000000002400,
        0x0000000000002400,
        0x0000000000000420,
        0x0000000000000420,
        0x0000000000002420,
        0x0000000000002420,
    ],
    [
        0x0000000000000000,
        0x2000000000000000,
        0x0000000400000000,
        0x2000000400000000,
        0x0000800000000000,
        0x2000800000000000,
        0x0000800400000000,
        0x2000800400000000,
        0x0008000000000000,
        0x2008000000000000,
        0x0008000400000000,
        0x2008000400000000,
        0x0008800000000000,
        0x2008800000000000,
        0x0008800400000000,
        0x2008800400000000,
    ],
];

// Tables adapted from pwhash DES implementation.
const PC2ROT: [[[u64; 16]; 16]; 2] = include!("./unix_crypt_compat_pc2rot.in");
const IE3264: [[u64; 16]; 8] = include!("./unix_crypt_compat_ie3264.in");
const CF6464: [[u64; 16]; 16] = include!("./unix_crypt_compat_cf6464.in");
const SPE: [[u64; 64]; 8] = include!("./unix_crypt_compat_spe.in");

#[allow(non_upper_case_globals)]
const Rotates: [usize; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

#[allow(non_snake_case)]
pub fn des_cipher(input: u64, keyword: u64, salt: u32, mut num_iter: u32) -> u64 {
    let salt = ((salt << 26) & 0xFC000000)
        | ((salt << 12) & 0xFC0000)
        | ((salt >> 2) & 0xFC00)
        | ((salt >> 16) & 0xFC);
    let mut l = input;
    let mut r = l;
    l &= 0x5555555555555555;
    r = (r & 0xAAAAAAAA00000000) | ((r >> 1) & 0x0000000055555555);
    l = (((l << 1) | (l << 32)) & 0xFFFFFFFF00000000) | ((r | (r >> 32)) & 0x00000000FFFFFFFF);

    fn perm3264(mut c: u32, p: &[[u64; 16]; 8]) -> u64 {
        let mut out = 0u64;
        let mut i = 3;
        while i >= 0 {
            let t = c & 0xFF;
            c >>= 8;
            let tp = p[(i << 1) as usize][(t & 0xF) as usize];
            out |= tp;
            let tp = p[(i << 1) as usize + 1][(t >> 4) as usize];
            out |= tp;
            i -= 1;
        }
        out
    }

    r = perm3264((l & 0xFFFFFFFF) as u32, &IE3264);
    l = perm3264((l >> 32) as u32, &IE3264);

    fn perm6464(mut c: u64, p: &[[u64; 16]; 16]) -> u64 {
        let mut out = 0u64;
        let mut i = 7;
        while i >= 0 {
            let t = (c & 0xFF) as u32;
            c >>= 8;
            let tp = p[(i << 1) as usize][(t & 0xF) as usize];
            out |= tp;
            let tp = p[(i << 1) as usize + 1][(t >> 4) as usize];
            out |= tp;
            i -= 1;
        }
        out
    }

    fn des_setkey(keyword: u64) -> [u64; 16] {
        let mut ks = [0u64; 16];
        let mut k = perm6464(keyword, &PC1ROT);
        ks[0] = k & !0x0303030300000000;
        for i in 1..16 {
            k = perm6464(k, &PC2ROT[Rotates[i] - 1]);
            ks[i] = k & !0x0303030300000000;
        }
        ks
    }

    let ks = des_setkey(keyword);

    while num_iter > 0 {
        num_iter -= 1;
        for loop_count in 0..8 {
            let kp = ks[loop_count << 1];
            let mut k = ((r >> 32) ^ r) & salt as u64 & 0xFFFFFFFF;
            k |= k << 32;
            let b = k ^ r ^ kp;
            l ^= SPE[0][((b >> 58) & 0x3F) as usize]
                ^ SPE[1][((b >> 50) & 0x3F) as usize]
                ^ SPE[2][((b >> 42) & 0x3F) as usize]
                ^ SPE[3][((b >> 34) & 0x3F) as usize]
                ^ SPE[4][((b >> 26) & 0x3F) as usize]
                ^ SPE[5][((b >> 18) & 0x3F) as usize]
                ^ SPE[6][((b >> 10) & 0x3F) as usize]
                ^ SPE[7][((b >> 2) & 0x3F) as usize];
            let kp = ks[(loop_count << 1) + 1];
            k = ((l >> 32) ^ l) & salt as u64 & 0xFFFFFFFF;
            k |= k << 32;
            let b = k ^ l ^ kp;
            r ^= SPE[0][((b >> 58) & 0x3F) as usize]
                ^ SPE[1][((b >> 50) & 0x3F) as usize]
                ^ SPE[2][((b >> 42) & 0x3F) as usize]
                ^ SPE[3][((b >> 34) & 0x3F) as usize]
                ^ SPE[4][((b >> 26) & 0x3F) as usize]
                ^ SPE[5][((b >> 18) & 0x3F) as usize]
                ^ SPE[6][((b >> 10) & 0x3F) as usize]
                ^ SPE[7][((b >> 2) & 0x3F) as usize];
        }
        std::mem::swap(&mut l, &mut r);
    }
    l = (((l >> 35) & 0x0F0F0F0F) | (((l & 0xFFFFFFFF) << 1) & 0xF0F0F0F0)) << 32
        | (((r >> 35) & 0x0F0F0F0F) | (((r & 0xFFFFFFFF) << 1) & 0xF0F0F0F0));
    perm6464(l, &CF6464)
}

#[cfg(test)]
mod tests {
    use super::crypt;

    #[test]
    fn known_value_matches() {
        assert_eq!(crypt(b"test", "aZ").unwrap(), "aZGJuE6EXrjEE");
    }

    #[test]
    fn invalid_salt_is_rejected() {
        assert!(crypt(b"test", "!!").is_err());
        assert!(crypt(b"test", "Z").is_err());
    }
}
