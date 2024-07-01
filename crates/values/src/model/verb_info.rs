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

use crate::encode::{DecodingError, EncodingError};
use crate::model::verbdef::VerbDef;
use crate::AsByteBuffer;
use bincode::de::{BorrowDecoder, Decoder};
use bincode::enc::Encoder;
use bincode::error::{DecodeError, EncodeError};
use bincode::{BorrowDecode, Decode, Encode};
use bytes::{BufMut, Bytes};
use std::convert::TryInto;

/// The binding of both a `VerbDef` and the binary (program) associated with it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerbInfo(Bytes);

impl VerbInfo {
    #[must_use]
    pub fn new(verbdef: VerbDef, binary: Bytes) -> Self {
        let mut storage = Vec::new();
        // Both values we hold are variable sized, but the program is always at the end, so we can
        // just append it without a length prefix.
        verbdef
            .with_byte_buffer(|buffer| {
                storage.put_u32_le(buffer.len() as u32);
                storage.put_slice(buffer);
            })
            .expect("Failed to encode verbdef");
        storage.put(binary.as_ref());
        Self(Bytes::from(storage))
    }

    #[must_use]
    pub fn verbdef(&self) -> VerbDef {
        let vd_len = u32::from_le_bytes(self.0[0..4].try_into().unwrap()) as usize;
        VerbDef::from_bytes(self.0.slice(4..4 + vd_len)).expect("Failed to decode verbdef")
    }

    #[must_use]
    pub fn binary(&self) -> Bytes {
        // The binary is after the verbdef, which is prefixed with a u32 length.
        let vd_len = u32::from_le_bytes(self.0[0..4].try_into().unwrap()) as usize;
        self.0.slice(4 + vd_len..)
    }
}

impl Encode for VerbInfo {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        let bytes = self.0.to_vec();
        bytes.encode(encoder)
    }
}

impl Decode for VerbInfo {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let bytes_vec: Vec<u8> = Vec::decode(decoder)?;
        Ok(Self(Bytes::from(bytes_vec)))
    }
}

impl<'de> BorrowDecode<'de> for VerbInfo {
    fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let bytes_vec: Vec<u8> = Vec::borrow_decode(decoder)?;
        Ok(Self(Bytes::from(bytes_vec)))
    }
}

impl AsByteBuffer for VerbInfo {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_ref()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.to_vec())
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, DecodingError> {
        // TODO validate
        Ok(Self(bytes))
    }

    fn as_bytes(&self) -> Result<Bytes, EncodingError> {
        Ok(self.0.clone())
    }
}
