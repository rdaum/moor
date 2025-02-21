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

extern crate core;

pub use encode::{
    AsByteBuffer, BINCODE_CONFIG, BincodeAsByteBufferExt, CountingWriter, DecodingError,
    EncodingError,
};
use shadow_rs::shadow;

pub use var::{
    AMBIGUOUS, Associative, ErrorPack, FAILED_MATCH, Flyweight, IndexMode, List, Map, NOTHING,
    SYSTEM_OBJECT, Sequence, Str, Var, Variant, v_bool_int, v_empty_list, v_empty_map, v_empty_str,
    v_err, v_float, v_flyweight, v_int, v_list, v_list_iter, v_map, v_map_iter, v_none, v_obj,
    v_objid, v_str, v_string, v_sym, v_sym_str,
};
pub use var::{Error, Obj, Symbol, VarType};

mod encode;
pub mod matching;
pub mod model;
pub mod tasks;
pub mod util;

mod var;

/// When encoding or decoding types to/from data or network, this is a version tag put into headers
/// for validity / version checking.
pub const DATA_LAYOUT_VERSION: u8 = 1;

shadow!(build);
