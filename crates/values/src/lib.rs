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
use crate::var::Objid;
use bincode::enc::write::Writer;
use bincode::error::EncodeError;
use bincode::{Decode, Encode};
use lazy_static::lazy_static;

pub mod model;
pub mod util;
pub mod var;

/// When encoding or decoding types to/from data or network, this is a version tag put into headers
/// for validity / version checking.
pub const DATA_LAYOUT_VERSION: u8 = 1;

/// The "system" object in MOO is a place where a bunch of basic sys functionality hangs off of, and
/// from where $name style references hang off of. A bit like the Lobby in Self.
pub const SYSTEM_OBJECT: Objid = Objid(0);

/// Used throughout to refer to a missing object value.
pub const NOTHING: Objid = Objid(-1);
/// Used in matching to indicate that the match was ambiguous on multiple objects in the
/// environment.
pub const AMBIGUOUS: Objid = Objid(-2);
/// Used in matching to indicate that the match failed to find any objects in the environment.
pub const FAILED_MATCH: Objid = Objid(-3);

/// A trait for all values that can be stored in the database. (e.g. all of them).
/// To abstract away from the underlying serialization format, we use this trait.
pub trait AsByteBuffer {
    /// Returns the size of this value in bytes.
    /// For now assume this is a costly operation.
    fn size_bytes(&self) -> usize;
    /// Return the bytes representing this value.
    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, f: F) -> R;
    // When you give up on zero-copy.
    fn make_copy_as_vec(&self) -> Vec<u8>;
    /// Create a value from the given bytes.
    /// Either takes ownership or moves.
    fn from_sliceref(bytes: SliceRef) -> Self;
    /// As a sliceref...
    fn as_sliceref(&self) -> SliceRef;
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

/// Implementation of `AsBytes` for all types that are binpackable.
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

    fn with_byte_buffer<R, F: FnMut(&[u8]) -> R>(&self, mut f: F) -> R
    where
        Self: Sized + Encode,
    {
        let v = bincode::encode_to_vec(self, *BINCODE_CONFIG).expect("bincode to bytes");
        f(&v[..])
    }

    fn make_copy_as_vec(&self) -> Vec<u8>
    where
        Self: Sized + Encode,
    {
        bincode::encode_to_vec(self, *BINCODE_CONFIG).expect("bincode to bytes")
    }

    fn from_sliceref(bytes: SliceRef) -> Self
    where
        Self: Sized + Decode,
    {
        bincode::decode_from_slice(bytes.as_slice(), *BINCODE_CONFIG)
            .expect("bincode from bytes")
            .0
    }

    fn as_sliceref(&self) -> SliceRef {
        SliceRef::from_vec(self.make_copy_as_vec())
    }
}
