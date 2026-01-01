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

//! StoredProgram - FlatBuffer-backed serializable representation of a Program.
//!
//! This wraps a FlatBuffer byte array with accessor methods, similar to how
//! VerbDef and PropDef work. The actual wire format is defined in
//! crates/common/schema/moor_program.fbs
//!
//! Flow:
//!   Disk → StoredProgram (ByteView) → decode → Program (runtime) → Execute
//!   (and same in reverse)

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

impl AsRef<ByteView> for StoredProgram {
    fn as_ref(&self) -> &ByteView {
        &self.0
    }
}

impl From<ByteView> for StoredProgram {
    fn from(bytes: ByteView) -> Self {
        Self(bytes)
    }
}
