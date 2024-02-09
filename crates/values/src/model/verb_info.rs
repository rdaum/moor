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
use crate::util::SliceRef;
use crate::AsByteBuffer;
use bytes::BufMut;
use std::convert::TryInto;

/// The binding of both a `VerbDef` and the binary (program) associated with it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerbInfo(SliceRef);

impl VerbInfo {
    #[must_use]
    pub fn new(verbdef: VerbDef, binary: SliceRef) -> Self {
        let mut storage = Vec::new();
        // Both values we hold are variable sized, but the program is always at the end, so we can
        // just append it without a length prefix.
        verbdef
            .with_byte_buffer(|buffer| {
                storage.put_u32_le(buffer.len() as u32);
                storage.put_slice(buffer);
            })
            .expect("Failed to encode verbdef");
        storage.put_slice(binary.as_slice());
        Self(SliceRef::from_bytes(&storage))
    }

    #[must_use]
    pub fn verbdef(&self) -> VerbDef {
        let vd_len = u32::from_le_bytes(self.0.as_slice()[0..4].try_into().unwrap()) as usize;
        VerbDef::from_sliceref(self.0.slice(4..4 + vd_len)).expect("Failed to decode verbdef")
    }

    #[must_use]
    pub fn binary(&self) -> SliceRef {
        // The binary is after the verbdef, which is prefixed with a u32 length.
        let vd_len = u32::from_le_bytes(self.0.as_slice()[0..4].try_into().unwrap()) as usize;
        self.0.slice(4 + vd_len..)
    }
}

impl AsByteBuffer for VerbInfo {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError> {
        Ok(f(self.0.as_slice()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError> {
        Ok(self.0.as_slice().to_vec())
    }

    fn from_sliceref(bytes: SliceRef) -> Result<Self, DecodingError> {
        // TODO validate
        Ok(Self(bytes))
    }

    fn as_sliceref(&self) -> Result<SliceRef, EncodingError> {
        Ok(self.0.clone())
    }
}
