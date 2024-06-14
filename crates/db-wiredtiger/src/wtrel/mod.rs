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

//! Defines a quasi-binary-relational API for WiredTiger tables over the WT bindings.

use std::rc::Rc;

use bytes::Bytes;
use moor_values::AsByteBuffer;

use crate::bindings::FormatType::RawByte;
use crate::bindings::{Datum, Pack, Session, Unpack};

pub mod rel_db;
pub mod rel_transaction;
pub mod relation;

// TODO: find ways of avoiding copies by using Bytes / ByteSource
fn to_datum<V: AsByteBuffer>(session: &Session, v: &V) -> Datum {
    v.with_byte_buffer(|bytes| {
        let mut pack = Pack::new(session, &[RawByte(None)], bytes.len());
        pack.push_item(bytes);
        pack.pack()
    })
    .unwrap()
}

fn from_datum<V: AsByteBuffer>(session: &Session, d: Rc<Datum>) -> V {
    let mut unpack = Unpack::new(session, &[RawByte(None)], d);
    let bytes = unpack.unpack_item();
    V::from_bytes(Bytes::from(bytes)).unwrap()
}
