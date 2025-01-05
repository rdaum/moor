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

extern crate core;

pub use encode::{
    AsByteBuffer, BincodeAsByteBufferExt, CountingWriter, DecodingError, EncodingError,
    BINCODE_CONFIG,
};

pub use var::{
    v_bool, v_empty_list, v_empty_map, v_empty_str, v_err, v_float, v_flyweight, v_int, v_list,
    v_list_iter, v_map, v_map_iter, v_none, v_obj, v_objid, v_str, v_string, Associative,
    ErrorPack, Flyweight, IndexMode, List, Map, Sequence, Str, Var, Variant, AMBIGUOUS,
    FAILED_MATCH, NOTHING, SYSTEM_OBJECT,
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
