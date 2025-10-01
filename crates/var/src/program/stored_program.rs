// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

//! StoredProgram - FlatBuffer-backed serializable representation of a Program.
//!
//! This wraps a FlatBuffer byte array with accessor methods, similar to how
//! VerbDef and PropDef work. The actual wire format is defined in
//! crates/common/schema/moor_program.fbs
//!
//! Flow:
//!   Disk → StoredProgram (ByteView) → decode → Program (runtime) → Execute
//!                                               ↓
//!                                            encode

use crate::AsByteBuffer;
use byteview::ByteView;

/// StoredProgram wraps a FlatBuffer representation of a program
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredProgram(ByteView);

impl StoredProgram {
    /// Create a StoredProgram from FlatBuffer bytes
    pub fn from_bytes(bytes: ByteView) -> Self {
        Self(bytes)
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    // Accessor methods are not provided here to avoid circular dependency with moor-common.
    // Decoding happens in moor-compiler via the `stored_to_program` function.
}

impl AsByteBuffer for StoredProgram {
    fn size_bytes(&self) -> usize {
        self.0.len()
    }

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(
        &self,
        mut f: F,
    ) -> Result<R, crate::EncodingError> {
        Ok(f(self.0.as_ref()))
    }

    fn make_copy_as_vec(&self) -> Result<Vec<u8>, crate::EncodingError> {
        Ok(self.0.as_ref().to_vec())
    }

    fn from_bytes(bytes: ByteView) -> Result<Self, crate::DecodingError> {
        // We don't validate the FlatBuffer here to avoid circular dependency.
        // Validation happens during decoding in moor-compiler.
        Ok(Self(bytes))
    }

    fn as_bytes(&self) -> Result<ByteView, crate::EncodingError> {
        Ok(self.0.clone())
    }
}
