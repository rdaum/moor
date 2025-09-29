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

//! Helper functions for constructing FlatBuffer types from Rust types
//!
//! These helpers prioritize zero-copy and move semantics to avoid unnecessary allocations.

use crate::{
    AuthToken, ClientToken, WorkerToken,
    convert::{
        obj_to_flatbuffer_struct, objectref_to_flatbuffer_struct, symbol_to_flatbuffer_struct,
        var_to_flatbuffer_bytes,
    },
    flatbuffers_generated::moor_rpc,
};
use moor_common::model::ObjectRef;
use moor_var::{Obj, Symbol, Var};

/// Create a FlatBuffer ClientToken from a reference (avoids moving the token)
#[inline]
pub fn client_token_fb(token: &ClientToken) -> Box<moor_rpc::ClientToken> {
    Box::new(moor_rpc::ClientToken {
        token: token.0.clone(),
    })
}

/// Create a FlatBuffer AuthToken from a reference (avoids moving the token)
#[inline]
pub fn auth_token_fb(token: &AuthToken) -> Box<moor_rpc::AuthToken> {
    Box::new(moor_rpc::AuthToken {
        token: token.0.clone(),
    })
}

/// Create a FlatBuffer WorkerToken from a reference (avoids moving the token)
#[inline]
pub fn mk_worker_token(token: &WorkerToken) -> Box<moor_rpc::WorkerToken> {
    Box::new(moor_rpc::WorkerToken {
        token: token.0.clone(),
    })
}

/// Create a boxed FlatBuffer Obj struct from an Obj reference
#[inline]
pub fn obj_fb(obj: &Obj) -> Box<moor_rpc::Obj> {
    Box::new(obj_to_flatbuffer_struct(obj))
}

/// Create a boxed FlatBuffer Symbol struct from a Symbol reference
#[inline]
pub fn symbol_fb(symbol: &Symbol) -> Box<moor_rpc::Symbol> {
    Box::new(symbol_to_flatbuffer_struct(symbol))
}

/// Create a boxed FlatBuffer ObjectRef struct from an ObjectRef reference
#[inline]
pub fn objectref_fb(objref: &ObjectRef) -> Box<moor_rpc::ObjectRef> {
    Box::new(objectref_to_flatbuffer_struct(objref))
}

/// Create a boxed FlatBuffer VarBytes from a Var reference
/// Returns None if serialization fails
#[inline]
pub fn var_fb(var: &Var) -> Option<Box<moor_rpc::VarBytes>> {
    Some(Box::new(moor_rpc::VarBytes {
        data: var_to_flatbuffer_bytes(var).ok()?,
    }))
}

/// Create a boxed FlatBuffer Uuid from a uuid::Uuid
#[inline]
pub fn uuid_fb(uuid: uuid::Uuid) -> Box<moor_rpc::Uuid> {
    Box::new(moor_rpc::Uuid {
        data: uuid.as_bytes().to_vec(),
    })
}

/// Create a FlatBuffer string list from an iterator of strings (moves strings)
#[inline]
pub fn string_list_fb(strings: impl IntoIterator<Item = String>) -> Vec<String> {
    strings.into_iter().collect()
}

/// Create a FlatBuffer symbol list from an iterator of Symbols
#[inline]
pub fn symbol_list_fb(symbols: impl IntoIterator<Item = Symbol>) -> Vec<moor_rpc::Symbol> {
    symbols
        .into_iter()
        .map(|s| moor_rpc::Symbol {
            value: s.as_string(),
        })
        .collect()
}
