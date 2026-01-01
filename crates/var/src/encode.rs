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

/// A trait for all common that can be stored in the database. (e.g. all of them).
/// To abstract away from the underlying serialization format, we use this trait.
pub trait ByteSized {
    /// Returns the size of this value in bytes.
    /// For now assume this is a costly operation.
    fn size_bytes(&self) -> usize;
}
