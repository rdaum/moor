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

use crate::{AsByteBuffer, DecodingError, EncodingError, Var};
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use bytes::Bytes;
use flexbuffers::Buffer;
use std::ops::{Deref, Range};
use std::str::Utf8Error;

/// A wrapper around bytes::Bytes that implements the Buffer trait for flexbuffers.
#[derive(Clone)]
pub struct VarBuffer(pub Bytes);

impl Buffer for VarBuffer {
    type BufferString = String;

    fn slice(&self, range: Range<usize>) -> Option<Self> {
        if range.start > range.end {
            return None;
        }
        Some(Self(self.0.slice(range)))
    }

    fn shallow_copy(&self) -> Self {
        Self(self.0.clone())
    }

    fn empty() -> Self {
        Self(Bytes::new())
    }

    fn empty_str() -> Self::BufferString {
        "".to_string()
    }

    fn buffer_str(&self) -> Result<Self::BufferString, Utf8Error> {
        std::str::from_utf8(self.0.as_ref()).map(|s| s.to_string())
    }
}

impl Deref for VarBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Var {
    pub fn to_bytes(&self) -> Bytes {
        self.0.to_bytes()
    }
}

impl AsByteBuffer for Var {
    fn size_bytes(&self) -> usize {
        self.0.to_bytes().len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        let bytes = self.0.to_bytes();
        let buf = bytes.as_ref();
        Ok(f(buf))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.to_bytes().to_vec())
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError>
    where
        Self: Sized,
    {
        Ok(Var::from_bytes(bytes))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(self.to_bytes().clone())
    }
}

impl Encode for Var {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        let buf = self.to_bytes().to_vec();
        buf.encode(encoder)
    }
}

impl Decode for Var {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::<u8>::decode(decoder)?;
        let bytes = Bytes::from(vec);
        Ok(Var::from_bytes(bytes))
    }
}

impl<'de> BorrowDecode<'de> for Var {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let vec = Vec::<u8>::decode(decoder)?;
        let bytes = Bytes::from(vec);
        Ok(Var::from_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use crate::Error::E_TYPE;
    use crate::{v_err, v_float, v_int, v_list, v_map, v_objid, v_str, Objid, Var};

    #[test]
    fn pack_unpack_int() {
        let v = v_int(1);
        let bytes = v.to_bytes();
        let v2 = Var::from_bytes(bytes);
        assert_eq!(v, v2);
    }

    #[test]
    fn pack_unpack_float() {
        let v = v_float(42.42);
        let bytes = v.to_bytes();
        let v2 = Var::from_bytes(bytes);
        assert_eq!(v, v2);
    }

    #[test]
    fn pack_unpack_error() {
        let v = v_err(E_TYPE);
        let bytes = v.to_bytes();
        let v2 = Var::from_bytes(bytes);
        assert_eq!(v, v2);
    }
    #[test]
    fn pack_unpack_string() {
        let v = v_str("hello");
        let bytes = v.to_bytes();
        let v2 = Var::from_bytes(bytes);
        assert_eq!(v, v2);
    }

    #[test]
    fn pack_unpack_objid() {
        let v = v_objid(Objid(1));
        let bytes = v.to_bytes();
        let v2 = Var::from_bytes(bytes);
        assert_eq!(v, v2);
    }
    #[test]
    fn pack_unpack_list() {
        let l = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let bytes = l.to_bytes();
        let l2 = Var::from_bytes(bytes);
        assert_eq!(l, l2);
    }

    #[test]
    fn pack_unpack_list_nested() {
        let l = v_list(&[v_int(1), v_int(2), v_int(3)]);
        let l2 = v_list(&[v_int(1), l.clone(), v_int(3)]);
        let bytes = l2.to_bytes();
        let l3 = Var::from_bytes(bytes);
        assert_eq!(l2, l3);
    }

    #[test]
    fn pack_unpack_map() {
        let m = v_map(&[(v_int(1), v_int(2)), (v_int(3), v_int(4))]);
        let bytes = m.to_bytes();
        let m2 = Var::from_bytes(bytes);
        assert_eq!(m, m2);
    }

    #[test]
    fn pack_unpack_map_nested() {
        let m = v_map(&[(v_int(1), v_int(2)), (v_int(3), v_int(4))]);
        let m2 = v_map(&[(v_int(1), m.clone()), (v_int(3), m.clone())]);
        let bytes = m2.to_bytes();
        let m3 = Var::from_bytes(bytes);
        assert_eq!(m2, m3);
    }
}
