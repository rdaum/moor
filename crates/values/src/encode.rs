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

use crate::util::SliceRef;
use bincode::enc::write::Writer;
use bincode::error::EncodeError;
use bincode::{Decode, Encode};
use lazy_static::lazy_static;

#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("Could not encode: {0}")]
    CouldNotEncode(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DecodingError {
    #[error("Could not decode: {0}")]
    CouldNotDecode(String),
    #[error("Invalid ArgSpec value: {0}")]
    InvalidArgSpecValue(u8),
    #[error("Invalid Preposition value: {0}")]
    InvalidPrepValue(i16),
    #[error("Invalid VerbFlag value: {0}")]
    InvalidVerbFlagValue(u8),
    #[error("Invalid BinartType value: {0}")]
    InvalidBinaryTypeValue(u8),
    #[error("Invalid Error value: {0}")]
    InvalidErrorValue(u8),
}

/// A trait for all values that can be stored in the database. (e.g. all of them).
/// To abstract away from the underlying serialization format, we use this trait.
pub trait AsByteBuffer {
    /// Returns the size of this value in bytes.
    /// For now assume this is a costly operation.
    fn size_bytes(&self) -> usize;
    /// Return the bytes representing this value.
    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, f: F) -> Result<R, EncodingError>;
    // When you give up on zero-copy.
    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError>;
    /// Create a value from the given bytes.
    /// Either takes ownership or moves.
    fn from_sliceref(bytes: SliceRef) -> Result<Self, DecodingError>
    where
        Self: Sized;
    /// As a sliceref...
    fn as_sliceref(&self) -> Result<SliceRef, EncodingError>;
}

lazy_static! {
    static ref BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();
}

struct CountingWriter {
    count: usize,
}
impl Writer for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.count += bytes.len();
        Ok(())
    }
}

/// Implementation of `AsByteBuffer` for all types that are binpackable.
impl<T: Encode + Decode + Sized> AsByteBuffer for T {
    fn size_bytes(&self) -> usize
    where
        Self: Encode,
    {
        // For now be careful with this as we have to bincode the whole thing in order to calculate
        // this. In the long run with a zero-copy implementation we can just return the size of the
        // underlying bytes.
        let mut cw = CountingWriter { count: 0 };
        bincode::encode_into_writer(self, &mut cw, *BINCODE_CONFIG)
            .expect("bincode to bytes for counting size");
        cw.count
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> Result<R, EncodingError>
    where
        Self: Sized + Encode,
    {
        let v = bincode::encode_to_vec(self, *BINCODE_CONFIG)
            .map_err(|e| EncodingError::CouldNotEncode(e.to_string()))?;
        Ok(f(&v[..]))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, EncodingError>
    where
        Self: Sized + Encode,
    {
        bincode::encode_to_vec(self, *BINCODE_CONFIG)
            .map_err(|e| EncodingError::CouldNotEncode(e.to_string()))
    }

    fn from_sliceref(bytes: SliceRef) -> Result<Self, DecodingError>
    where
        Self: Sized + Decode,
    {
        Ok(
            bincode::decode_from_slice(bytes.as_slice(), *BINCODE_CONFIG)
                .map_err(|e| DecodingError::CouldNotDecode(e.to_string()))?
                .0,
        )
    }

    fn as_sliceref(&self) -> Result<SliceRef, EncodingError> {
        Ok(SliceRef::from_vec(self.make_copy_as_vec()?))
    }
}
