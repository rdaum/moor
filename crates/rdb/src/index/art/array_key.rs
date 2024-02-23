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

use std::mem;

use crate::index::art::array_partial::ArrPartial;
use crate::index::art::{KeyTrait, Partial};

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ArrayKey<const N: usize> {
    data: [u8; N],
    len: usize,
}

impl<const N: usize> ArrayKey<N> {
    pub fn new_from_str(s: &str) -> Self {
        assert!(s.len() + 1 < N, "data length is greater than array length");
        let mut arr = [0; N];
        arr[..s.len()].copy_from_slice(s.as_bytes());
        Self {
            data: arr,
            len: s.len() + 1,
        }
    }

    pub fn new_from_string(s: &String) -> Self {
        assert!(s.len() + 1 < N, "data length is greater than array length");
        let mut arr = [0; N];
        arr[..s.len()].copy_from_slice(s.as_bytes());
        Self {
            data: arr,
            len: s.len() + 1,
        }
    }

    #[allow(dead_code)]
    pub fn new_from_array<const S: usize>(arr: [u8; S]) -> Self {
        Self::new_from_slice(&arr)
    }

    #[allow(dead_code)]
    pub fn as_array(&self) -> &[u8; N] {
        &self.data
    }

    #[allow(dead_code)]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// (Convenience function. Not all keys can be assumed to be numeric.)
    pub fn to_be_u64(&self) -> u64 {
        // Copy from 0..min(len, 8) to a new array left-padding it, then convert to u64.
        let mut arr = [0; 8];
        arr[8 - self.len..].copy_from_slice(&self.data[..self.len]);
        u64::from_be_bytes(arr)
    }
}

impl<const N: usize> KeyTrait for ArrayKey<N> {
    type PartialType = ArrPartial<N>;
    const MAXIMUM_SIZE: Option<usize> = Some(N);

    fn new_from_slice(data: &[u8]) -> Self {
        assert!(data.len() <= N, "data length is greater than array length");
        let mut arr = [0; N];
        arr[0..data.len()].copy_from_slice(data);
        Self {
            data: arr,
            len: data.len(),
        }
    }

    fn new_from_partial(partial: &Self::PartialType) -> Self {
        let mut data = [0; N];
        let len = partial.len();
        data[..len].copy_from_slice(&partial.to_slice()[..len]);
        Self { data, len }
    }

    fn terminate_with_partial(&self, partial: &Self::PartialType) -> Self {
        let cur_len = self.len;
        let partial_len = partial.len();
        assert!(
            cur_len + partial_len <= N,
            "data length is greater than max key length"
        );
        let mut data = [0; N];
        data[..cur_len].copy_from_slice(&self.data[..cur_len]);
        let partial_slice = partial.to_slice();
        data[cur_len..cur_len + partial_len].copy_from_slice(&partial_slice[..partial_len]);
        Self {
            data,
            len: cur_len + partial_len,
        }
    }

    fn extend_from_partial(&self, partial: &Self::PartialType) -> Self {
        let cur_len = self.len;
        let partial_len = partial.len();
        assert!(
            cur_len + partial_len <= N,
            "data length is greater than max key length"
        );
        let mut data = [0; N];
        data[..cur_len].copy_from_slice(&self.data[..cur_len]);
        let partial_slice = partial.to_slice();
        data[cur_len..cur_len + partial_len].copy_from_slice(&partial_slice[..partial_len]);
        Self {
            data,
            len: cur_len + partial_len,
        }
    }

    fn truncate(&self, at_depth: usize) -> Self {
        assert!(at_depth <= self.len, "truncating beyond key length");
        Self {
            data: self.data,
            len: at_depth,
        }
    }

    #[inline(always)]
    fn at(&self, pos: usize) -> u8 {
        self.data[pos]
    }
    #[inline(always)]
    fn length_at(&self, at_depth: usize) -> usize {
        self.len - at_depth
    }
    fn to_partial(&self, at_depth: usize) -> ArrPartial<N> {
        ArrPartial::from_slice(&self.data[at_depth..self.len])
    }
    #[inline(always)]
    fn matches_slice(&self, slice: &[u8]) -> bool {
        &self.data[..self.len] == slice
    }
}

impl<const N: usize> From<String> for ArrayKey<N> {
    fn from(data: String) -> Self {
        Self::new_from_string(&data)
    }
}
impl<const N: usize> From<&String> for ArrayKey<N> {
    fn from(data: &String) -> Self {
        Self::new_from_string(data)
    }
}
impl<const N: usize> From<&str> for ArrayKey<N> {
    fn from(data: &str) -> Self {
        Self::new_from_str(data)
    }
}
macro_rules! impl_from_unsigned {
    ( $($t:ty),* ) => {
    $(
    impl<const N: usize> From< $t > for ArrayKey<N>
    {
        fn from(data: $t) -> Self {
            Self::new_from_slice(data.to_be_bytes().as_ref())
        }
    }
    impl<const N: usize> From< &$t > for ArrayKey<N>
    {
        fn from(data: &$t) -> Self {
            Self::new_from_slice(data.to_be_bytes().as_ref())
        }
    }
    ) *
    }
}
impl_from_unsigned!(u8, u16, u32, u64, usize, u128);

impl<const N: usize> From<i8> for ArrayKey<N> {
    fn from(val: i8) -> Self {
        let v: u8 = unsafe { mem::transmute(val) };
        let i = (v ^ 0x80) & 0x80;
        let j = i | (v & 0x7F);
        let mut data = [0; N];
        data[0] = j;
        Self { data, len: 1 }
    }
}

macro_rules! impl_from_signed {
    ( $t:ty, $tu:ty ) => {
        impl<const N: usize> From<$t> for ArrayKey<N> {
            fn from(val: $t) -> Self {
                let v: $tu = unsafe { mem::transmute(val) };
                let xor = 1 << (std::mem::size_of::<$tu>() - 1);
                let i = (v ^ xor) & xor;
                let j = i | (v & (<$tu>::MAX >> 1));
                ArrayKey::new_from_slice(j.to_be_bytes().as_ref())
            }
        }

        impl<const N: usize> From<&$t> for ArrayKey<N> {
            fn from(val: &$t) -> Self {
                (*val).into()
            }
        }
    };
}

impl_from_signed!(i16, u16);
impl_from_signed!(i32, u32);
impl_from_signed!(i64, u64);
impl_from_signed!(i128, u128);
impl_from_signed!(isize, usize);

#[cfg(test)]
mod test {
    use crate::index::art::array_key::ArrayKey;
    use crate::index::art::array_partial::ArrPartial;
    use crate::index::art::KeyTrait;

    #[test]
    fn make_extend_truncate() {
        let k = ArrayKey::<8>::new_from_slice(b"hel");
        let p = ArrPartial::<8>::from_slice(b"lo");
        let k2 = k.extend_from_partial(&p);
        assert!(k2.matches_slice(b"hello"));
        let k3 = k2.truncate(3);
        assert!(k3.matches_slice(b"hel"));
    }

    #[test]
    fn from_to_u64() {
        let k: ArrayKey<16> = 123u64.into();
        assert_eq!(k.to_be_u64(), 123u64);

        let k: ArrayKey<16> = 1u64.into();
        assert_eq!(k.to_be_u64(), 1u64);

        let k: ArrayKey<16> = 123213123123123u64.into();
        assert_eq!(k.to_be_u64(), 123213123123123u64);
    }
}
