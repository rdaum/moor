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

use bincode::enc::write::Writer;
use bincode::error::EncodeError;
use daumtils::SliceRef;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref BINCODE_CONFIG: bincode::config::Configuration = bincode::config::standard();
}

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

pub struct CountingWriter {
    pub count: usize,
}
impl Writer for CountingWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.count += bytes.len();
        Ok(())
    }
}
